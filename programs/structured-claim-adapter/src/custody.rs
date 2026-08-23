//! Ephemeral typed custody authority and exact General V2 action-35 CPI plan.

use clutch_collateral_adapter_v2::{
    BoundCollateralProfileV2, ClaimLedgerV3, CollateralPolicyV2, HoardV2,
    MarketLiabilityLifecycleV1,
};
use clutch_general_v2_contract::{
    project_general_position_replay_prestate_v1, project_general_replay_transition_v1,
    GeneralReplayTransitionKindV1, Id32, MarketBindingV1, MarketRuntimeV3AccountV1,
    GENERAL_REPLAY_ACCOUNT_V1_BYTES, GENERAL_REPLAY_EXTENSION_SCHEMA_V1,
};
use clutch_owner_settlement::{AuthenticatedPositionV3, PositionSettlementPoststateV3};
use clutch_product_series::{
    FixedCodec, MarketInstancePreimageV2, NativeClaimBasisV1, NATIVE_CLAIM_BASIS_DOMAIN,
};
use clutch_retirement::{
    Identity32V1, PositionAccountV3, PositionLifecycleV3, PositionPurposeV3,
    PositionV3Sha256Backend, ReplayV3Envelope, ReplayV3HashBackend, ReplayV3Lifecycle,
};
use clutch_retirement_adapter::{
    authenticate_position_v3_exact, authenticate_purpose_replay_v3_exact,
    AccountAccessV2 as RetirementAccountAccessV2, AccountViewV2 as RetirementAccountViewV2,
    CanonicalPdaV1,
};
use clutch_solana_layout::{
    registry::{ExtensionAction, ExtensionEnvelope, ExtensionFamily, GeneralV2Action},
    ProfileAccount, RealmAccount,
};
use clutch_solana_reference::ExtensionRequest;
use clutch_structured_claim::MarketPhase;
#[cfg(not(target_os = "solana"))]
use sha2::{Digest, Sha256};

use crate::runtime_contract::{
    prepare_atomic_position_asset_transfer_v1, AssetTransferPhasePolicyV1,
    AtomicPositionAssetTransferRequestV1, DescriptorStateV1, PositionAssetTransferAuthorityKindV1,
    PositionAssetTransferPayloadV1, PositionProjectionV1, StructuredClaimActionV1,
    StructuredClaimReplayDeltaV1, StructuredClaimReplayExtensionV1,
    StructuredClaimReplayTransitionV1, StructuredCustodyCallProjectionV1,
    POSITION_ASSET_TRANSFER_PAYLOAD_BYTES, STRUCTURED_CLAIM_REPLAY_DELTA_DOMAIN_V1,
    STRUCTURED_CLAIM_REPLAY_EXTENSION_SCHEMA_V1, STRUCTURED_CUSTODY_CALL_PREIMAGE_BYTES,
    STRUCTURED_CUSTODY_CALL_V1_DOMAIN,
};
use crate::{
    decode_owned_descriptor_v1, is_zero, AccountRoleV1, BasePositionPdaVerifierV1,
    BoundDescriptorV1, Error, Key, RawAccountV1, Result, RuntimeDeploymentsV1,
};

/// Digest domain for an exact canonical live descriptor body.
pub const STRUCTURED_CUSTODY_DESCRIPTOR_BODY_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-custody/descriptor-body/v1\0";
/// Digest domain for an exact full-width Hoard V2 prestate.
pub const STRUCTURED_CUSTODY_HOARD_BODY_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-custody/hoard-v2-body/v1\0";
/// Digest domain for an exact canonical General V2 MarketBinding body.
pub const STRUCTURED_CUSTODY_MARKET_BINDING_BODY_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-custody/market-binding-body/v1\0";
/// Digest domain for an exact canonical General V2 MarketRuntime body.
pub const STRUCTURED_CUSTODY_MARKET_RUNTIME_BODY_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-custody/market-runtime-body/v1\0";
/// Digest domain for an exact full-width ClaimLedger V3 prestate.
pub const STRUCTURED_CUSTODY_CLAIM_LEDGER_BODY_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-custody/claim-ledger-v3-body/v1\0";
/// Exact account count for structured custody through General action 35.
pub const STRUCTURED_CUSTODY_ACCOUNT_COUNT: usize = 23;
const IX_VAULT_AUTHORITY: usize = 0;
const IX_REALM: usize = 1;
const IX_PROFILE: usize = 2;
const IX_COLLATERAL_POLICY: usize = 3;
const IX_COLLATERAL_TOKEN_PROGRAM: usize = 4;
const IX_MARKET_BINDING: usize = 5;
const IX_MARKET_RUNTIME: usize = 6;
const IX_SOURCE_POSITION: usize = 7;
const IX_SOURCE_REPLAY: usize = 8;
const IX_DESTINATION_POSITION: usize = 9;
const IX_DESTINATION_REPLAY: usize = 10;
const IX_ACTOR: usize = 11;
const IX_DESCRIPTOR: usize = 12;
const IX_WRAPPER_PROGRAM: usize = 13;
const IX_WRAPPER_PROGRAM_DATA: usize = 14;
const IX_BASE_PROGRAM: usize = 15;
const IX_BASE_PROGRAM_DATA: usize = 16;
const IX_WRAPPER_TOKEN_2022_PROGRAM: usize = 17;
const IX_WRAPPER_TOKEN_2022_PROGRAM_DATA: usize = 18;
const IX_NATIVE_CLAIM_BASIS: usize = 19;
const IX_MARKET_INSTANCE: usize = 20;
const IX_HOARD_V2: usize = 21;
const IX_CLAIM_LEDGER_V3: usize = 22;
/// Exact outer base request, General family envelope, and 298-byte action body.
pub const BASE_POSITION_TRANSFER_CPI_BYTES: usize = 314;
/// Exact canonical Position V3 account width staged by action 35.
pub const POSITION_V3_WRITE_BYTES: usize = 480;
/// Largest Replay V3 body staged by this bridge (common prefix + SCV1).
pub const MAX_CUSTODY_REPLAY_V3_WRITE_BYTES: usize = 416;

/// Caller-owned large decode/hash storage for the 4-KiB SBF stack boundary.
///
/// An SBF entrypoint should place this value on its requestable heap and reuse
/// it for the wrapper-side preparation and base-side reconstruction. Keeping
/// these values here prevents a 2,352-byte Product decode plus a 1,480-byte
/// authority transcript from being returned through one call frame.
#[derive(Debug)]
pub struct StructuredCustodyScratchV1 {
    native_claim_basis: NativeClaimBasisV1,
    authority_preimage: [u8; STRUCTURED_CUSTODY_CALL_PREIMAGE_BYTES],
}

impl StructuredCustodyScratchV1 {
    /// Canonical empty scratch value; it carries no authenticated semantics.
    pub const ZEROED: Self = Self {
        native_claim_basis: NativeClaimBasisV1::ZEROED,
        authority_preimage: [0; STRUCTURED_CUSTODY_CALL_PREIMAGE_BYTES],
    };
}

/// One exact Solana CPI account meta without importing the Solana SDK.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpiAccountMetaV1 {
    /// Runtime account address.
    pub address: Key,
    /// Whether the callee observes a signer.
    pub signer: bool,
    /// Whether the callee may mutate the account.
    pub writable: bool,
}

impl CpiAccountMetaV1 {
    const EMPTY: Self = Self {
        address: [0; 32],
        signer: false,
        writable: false,
    };
}

/// Exact General V2 action-35 instruction and ordered CPI metas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BasePositionTransferCpiV1 {
    /// Exact base program selected by the immutable descriptor.
    pub program_id: Key,
    /// Canonical base `ExtensionRequest(sequence=0, GeneralV2/1/action35/payload298)`.
    pub data: [u8; BASE_POSITION_TRANSFER_CPI_BYTES],
    /// Frozen common prefix followed by every reconstruction input.
    pub accounts: [CpiAccountMetaV1; STRUCTURED_CUSTODY_ACCOUNT_COUNT],
}

/// One exact staged Position V3 compare-and-write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionV3WriteV1 {
    /// Exact writable Position account.
    pub address: Key,
    /// Semantic ID of the authenticated prestate.
    pub prestate_semantic_id: Key,
    /// Semantic ID of the exact successor body.
    pub poststate_semantic_id: Key,
    /// Exact canonical 480-byte successor body.
    pub body: [u8; POSITION_V3_WRITE_BYTES],
}

/// One exact staged purpose-owned Replay V3 compare-and-write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayV3WriteV1 {
    /// Exact writable Replay account.
    pub address: Key,
    /// Semantic ID of the authenticated prestate.
    pub prestate_semantic_id: Key,
    /// Semantic ID of the exact successor body.
    pub poststate_semantic_id: Key,
    /// Exact active body prefix length.
    pub body_len: u16,
    /// Exact successor body followed by canonical zero padding.
    pub body: [u8; MAX_CUSTODY_REPLAY_V3_WRITE_BYTES],
}

/// Atomic source/destination Position V3 and Replay V3 successor plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredCustodyPoststateV1 {
    /// Source Position successor.
    pub source_position: PositionV3WriteV1,
    /// Source purpose-owned Replay successor.
    pub source_replay: ReplayV3WriteV1,
    /// Destination Position successor.
    pub destination_position: PositionV3WriteV1,
    /// Destination purpose-owned Replay successor.
    pub destination_replay: ReplayV3WriteV1,
    /// Structured-purpose digest of both exact Position deltas and ordinals.
    pub structured_delta_id: Key,
    /// General-purpose digest of its exact Position delta and ordinal.
    pub general_delta_id: Key,
}

/// Target-specific PDA checks that cannot live in a pure codec.
///
/// An SBF implementation must derive every address with Solana's program
/// address primitive. Returning true from caller-authored booleans is not an
/// implementation of this boundary.
pub trait StructuredCustodyPdaVerifierV1: BasePositionPdaVerifierV1 {
    /// Verify the canonical immutable Realm PDA and stored bump.
    fn verify_realm(&self, base_program: Key, address: Key, realm_id: Key, stored_bump: u8)
        -> bool;

    /// Verify the canonical immutable Profile V2 PDA.
    fn verify_profile(
        &self,
        base_program: Key,
        address: Key,
        realm_id: Key,
        profile_id: Key,
    ) -> bool;

    /// Verify the canonical sealed collateral-policy artifact PDA.
    fn verify_collateral_policy(
        &self,
        base_program: Key,
        address: Key,
        profile_id: Key,
        policy_id: Key,
    ) -> bool;

    /// Parse the upgradeable-loader Program/ProgramData pair and verify its
    /// exact linkage, deployment slot, owner, executable, and read-only state.
    fn verify_upgradeable_deployment(
        &self,
        upgradeable_loader: Key,
        program: &RawAccountV1<'_>,
        program_data: &RawAccountV1<'_>,
        expected_deployment_slot: u64,
    ) -> bool;

    /// Verify `general-market-binding:v1 || MarketInstanceV2Id` and stored bump.
    fn verify_market_binding(
        &self,
        base_program: Key,
        address: Key,
        market_instance_id: Key,
        stored_bump: u8,
    ) -> bool;

    /// Verify the stable General V2 MarketRuntime selected by MarketBinding.
    fn verify_market_runtime(
        &self,
        base_program: Key,
        address: Key,
        market_binding: Key,
        stored_bump: u8,
    ) -> bool;

    /// Verify the canonical full-width Hoard V2 PDA.
    fn verify_hoard_v2(
        &self,
        base_program: Key,
        address: Key,
        market_instance_id: Key,
        stored_bump: u8,
    ) -> bool;

    /// Verify the canonical full-width ClaimLedger V3 PDA.
    fn verify_claim_ledger_v3(
        &self,
        base_program: Key,
        address: Key,
        market_instance_id: Key,
        stored_bump: u8,
    ) -> bool;

    /// Verify a base-owned Product artifact address for its exact kind and id.
    fn verify_product_artifact(
        &self,
        base_program: Key,
        address: Key,
        artifact_kind: u8,
        content_id: Key,
    ) -> bool;

    /// Verify the exact base-owned account carrying a MarketInstanceV2 preimage.
    fn verify_market_instance_artifact(
        &self,
        base_program: Key,
        address: Key,
        market_instance_id: Key,
    ) -> bool;
}

/// SHA-256 backend shared by host planning and SBF reconstruction.
#[derive(Clone, Copy, Debug, Default)]
pub struct AdapterSha256V1;

impl AdapterSha256V1 {
    fn hash(self, domain: &[u8], body: &[u8]) -> Key {
        #[cfg(target_os = "solana")]
        {
            solana_sha256_hasher::hashv(&[domain, body]).to_bytes()
        }
        #[cfg(not(target_os = "solana"))]
        {
            let mut hasher = Sha256::new();
            hasher.update(domain);
            hasher.update(body);
            hasher.finalize().into()
        }
    }
}

impl PositionV3Sha256Backend for AdapterSha256V1 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> Key {
        (*self).hash(domain, body)
    }
}

impl ReplayV3HashBackend for AdapterSha256V1 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> Key {
        #[cfg(target_os = "solana")]
        {
            solana_sha256_hasher::hashv(parts).to_bytes()
        }
        #[cfg(not(target_os = "solana"))]
        {
            let mut hasher = Sha256::new();
            let mut index = 0_usize;
            while index < parts.len() {
                hasher.update(parts[index]);
                index += 1;
            }
            hasher.finalize().into()
        }
    }
}

/// Private-field authority minted only by complete wrapper/base reconstruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedStructuredCustodyCallV1 {
    authority_id: Key,
    local_action: StructuredClaimActionV1,
    transfer: PositionAssetTransferPayloadV1,
    source_after: PositionProjectionV1,
    destination_after: PositionProjectionV1,
    poststate: StructuredCustodyPoststateV1,
    cpi: BasePositionTransferCpiV1,
}

impl AuthenticatedStructuredCustodyCallV1 {
    /// Exact domain-separated authority digest placed in action 35.
    pub const fn authority_id(&self) -> Key {
        self.authority_id
    }

    /// Exact wrapper action inferred from the authenticated endpoint purposes.
    pub const fn local_action(&self) -> StructuredClaimActionV1 {
        self.local_action
    }

    /// Exact final action-35 payload carrying this typed authority digest.
    pub const fn transfer(&self) -> PositionAssetTransferPayloadV1 {
        self.transfer
    }

    /// Runtime-owned source successor reconstructed from authoritative state.
    pub const fn source_after(&self) -> PositionProjectionV1 {
        self.source_after
    }

    /// Runtime-owned destination successor reconstructed from authoritative state.
    pub const fn destination_after(&self) -> PositionProjectionV1 {
        self.destination_after
    }

    /// Exact four-account compare-and-write poststate staged for the base.
    pub const fn poststate(&self) -> StructuredCustodyPoststateV1 {
        self.poststate
    }

    /// Exact CPI instruction bytes and ordered account metas.
    pub const fn cpi(&self) -> BasePositionTransferCpiV1 {
        self.cpi
    }
}

/// Reconstruct and authenticate one structured-custody Position V3 call.
///
/// Both the wrapper and General action-35 handler call this same function over
/// their independently authenticated account views. No rent-bearing generic
/// capability account exists: the vault PDA signer plus this exact digest is
/// the ephemeral typed authority.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_structured_custody_call_v1<P: StructuredCustodyPdaVerifierV1>(
    accounts: &[RawAccountV1<'_>],
    descriptor: &BoundDescriptorV1,
    deployments: RuntimeDeploymentsV1,
    collateral: BoundCollateralProfileV2,
    transfer: PositionAssetTransferPayloadV1,
    scratch: &mut StructuredCustodyScratchV1,
    verifier: &P,
) -> Result<AuthenticatedStructuredCustodyCallV1> {
    reconstruct_structured_custody_call_v1(
        accounts,
        descriptor,
        deployments,
        collateral,
        transfer,
        scratch,
        verifier,
        true,
    )
}

/// Prepare the wrapper side of a typed custody call from an authority-neutral payload.
///
/// The supplied `authority_id` must be zero. The returned call installs the
/// reconstructed digest into the final 298-byte payload and CPI bytes. The
/// base subsequently calls [`authenticate_structured_custody_call_v1`] over
/// the final payload and the same authenticated account projection.
#[allow(clippy::too_many_arguments)]
pub fn prepare_structured_custody_call_v1<P: StructuredCustodyPdaVerifierV1>(
    accounts: &[RawAccountV1<'_>],
    descriptor: &BoundDescriptorV1,
    deployments: RuntimeDeploymentsV1,
    collateral: BoundCollateralProfileV2,
    authority_neutral_transfer: PositionAssetTransferPayloadV1,
    scratch: &mut StructuredCustodyScratchV1,
    verifier: &P,
) -> Result<AuthenticatedStructuredCustodyCallV1> {
    if authority_neutral_transfer.authority_id != [0; 32] {
        return Err(Error::CustodyAuthorityMismatch);
    }
    reconstruct_structured_custody_call_v1(
        accounts,
        descriptor,
        deployments,
        collateral,
        authority_neutral_transfer,
        scratch,
        verifier,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn reconstruct_structured_custody_call_v1<P: StructuredCustodyPdaVerifierV1>(
    accounts: &[RawAccountV1<'_>],
    descriptor: &BoundDescriptorV1,
    deployments: RuntimeDeploymentsV1,
    collateral: BoundCollateralProfileV2,
    transfer: PositionAssetTransferPayloadV1,
    scratch: &mut StructuredCustodyScratchV1,
    verifier: &P,
    authenticate_final_digest: bool,
) -> Result<AuthenticatedStructuredCustodyCallV1> {
    validate_account_frame(accounts, descriptor, deployments)?;
    let descriptor_prestate = decode_owned_descriptor_v1(
        deployments.binding.wrapper_program,
        descriptor.addresses().descriptor,
        &accounts[IX_DESCRIPTOR],
    )?;
    if descriptor.descriptor().state != DescriptorStateV1::Active
        || descriptor_prestate != *descriptor.descriptor()
        || descriptor.identity().deployment != deployments.binding
    {
        return Err(Error::CustodyAuthorityMismatch);
    }
    let base_program = deployments.binding.base_program;
    let realm =
        RealmAccount::decode(accounts[IX_REALM].data).map_err(|_| Error::ProductBoundary)?;
    let profile =
        ProfileAccount::decode(accounts[IX_PROFILE].data).map_err(|_| Error::ProductBoundary)?;
    let collateral_policy = CollateralPolicyV2::decode(accounts[IX_COLLATERAL_POLICY].data)
        .map_err(|_| Error::ProductBoundary)?;
    let collateral_policy_id = collateral_policy
        .id()
        .map_err(|_| Error::ProductBoundary)?
        .bytes();
    let market_binding = MarketBindingV1::decode(accounts[IX_MARKET_BINDING].data)
        .map_err(|_| Error::ProductBoundary)?;
    let market_runtime = MarketRuntimeV3AccountV1::decode(accounts[IX_MARKET_RUNTIME].data)
        .map_err(|_| Error::ProductBoundary)?;
    NativeClaimBasisV1::decode_into(
        accounts[IX_NATIVE_CLAIM_BASIS].data,
        &mut scratch.native_claim_basis,
    )
    .map_err(|_| Error::ProductBoundary)?;
    let basis = &scratch.native_claim_basis;
    let market_instance = MarketInstancePreimageV2::decode(accounts[IX_MARKET_INSTANCE].data)
        .map_err(|_| Error::ProductBoundary)?;
    let hoard = HoardV2::decode(accounts[IX_HOARD_V2].data).map_err(|_| Error::ProductBoundary)?;
    let claim_ledger = ClaimLedgerV3::decode(accounts[IX_CLAIM_LEDGER_V3].data)
        .map_err(|_| Error::ProductBoundary)?;
    let sha = AdapterSha256V1;
    let basis_id = sha.hash(
        NATIVE_CLAIM_BASIS_DOMAIN,
        accounts[IX_NATIVE_CLAIM_BASIS].data,
    );
    let market_instance_id = market_instance
        .id()
        .map_err(|_| Error::ProductBoundary)?
        .bytes();
    if market_binding.market_instance_v2_id.bytes() != market_instance_id
        || market_binding.market_genesis_profile_v2_id.bytes()
            != market_instance
                .market_genesis_profile_id
                .content_id()
                .bytes()
        || market_binding.native_claim_basis_id.bytes() != basis_id
        || market_binding.outcome_count != basis.outcome_count
        || market_binding.basis_degree != basis.basis_degree
        || descriptor.descriptor().market != market_instance_id
        || descriptor.descriptor().terms_digest != basis_id
        || descriptor.identity().claim.basis.market != market_instance_id
        || descriptor.identity().claim.basis.terms != basis_id
        || descriptor.identity().claim.basis.basis_degree != basis.basis_degree
        || descriptor.identity().claim.basis.outcome_count != basis.outcome_count
        || descriptor.identity().claim.basis.denominator != basis.denominator
    {
        return Err(Error::ProductBoundary);
    }
    let collateral_market = collateral.market();
    let collateral_realm = collateral.realm_bound().realm();
    let collateral_release_id = collateral
        .release()
        .id()
        .map_err(|_| Error::ProductBoundary)?
        .bytes();
    if realm.realm.bytes() != collateral_realm.realm.bytes()
        || realm.profile.bytes() != collateral_realm.profile.bytes()
        || profile.realm != realm.realm
        || profile.profile != realm.profile
        || profile.collateral_policy_id.bytes() != collateral_policy_id
        || profile.adapter_release_id.bytes() != collateral_release_id
        || collateral.policy_id().bytes() != collateral_policy_id
        || collateral_policy.adapter_release.bytes() != collateral_release_id
        || accounts[IX_COLLATERAL_TOKEN_PROGRAM].key != collateral.release().token_program.bytes()
        || accounts[IX_WRAPPER_TOKEN_2022_PROGRAM].key != deployments.binding.token_2022_program
        || collateral_market.market.bytes() != market_instance_id
        || collateral_market.realm != collateral_realm.realm
        || collateral_market.profile != collateral_realm.profile
        || collateral_market.collateral_cap_atoms != market_instance.collateral_cap
        || market_runtime.market_binding.bytes() != accounts[IX_MARKET_BINDING].key
        || market_runtime.market_instance_v2_id.bytes() != market_instance_id
        || hoard.market_instance_id.bytes() != market_instance_id
        || hoard.realm_id.bytes() != realm.realm.bytes()
        || hoard.profile_id.bytes() != profile.profile.bytes()
        || hoard.collateral_policy_id.bytes() != collateral_policy_id
        || hoard.collateral_release_id.bytes() != collateral_release_id
        || hoard.authority != collateral_market.hoard_authority
        || hoard.token_account != collateral_market.hoard_token_account
        || hoard.collateral_cap_atoms != market_instance.collateral_cap
        || hoard.outcome_count != basis.outcome_count
        || claim_ledger.market_instance_id != hoard.market_instance_id
        || claim_ledger.realm_id != hoard.realm_id
        || claim_ledger.native_claim_basis_id.bytes() != basis_id
        || claim_ledger.lifecycle != hoard.lifecycle
        || claim_ledger.outcome_count != hoard.outcome_count
    {
        return Err(Error::ProductBoundary);
    }
    let market_phase = decode_market_phase(hoard.lifecycle)?;
    if !verifier.verify_upgradeable_deployment(
        deployments.upgradeable_loader,
        &accounts[IX_WRAPPER_PROGRAM],
        &accounts[IX_WRAPPER_PROGRAM_DATA],
        deployments.binding.wrapper_deployment_slot,
    ) || !verifier.verify_upgradeable_deployment(
        deployments.upgradeable_loader,
        &accounts[IX_BASE_PROGRAM],
        &accounts[IX_BASE_PROGRAM_DATA],
        deployments.binding.base_deployment_slot,
    ) || !verifier.verify_upgradeable_deployment(
        deployments.upgradeable_loader,
        &accounts[IX_WRAPPER_TOKEN_2022_PROGRAM],
        &accounts[IX_WRAPPER_TOKEN_2022_PROGRAM_DATA],
        deployments.binding.token_2022_deployment_slot,
    ) || !verifier.verify_realm(
        base_program,
        accounts[IX_REALM].key,
        realm.realm.bytes(),
        realm.stored_bump,
    ) || !verifier.verify_profile(
        base_program,
        accounts[IX_PROFILE].key,
        realm.realm.bytes(),
        profile.profile.bytes(),
    ) || !verifier.verify_collateral_policy(
        base_program,
        accounts[IX_COLLATERAL_POLICY].key,
        profile.profile.bytes(),
        collateral_policy_id,
    ) || !verifier.verify_market_binding(
        base_program,
        accounts[IX_MARKET_BINDING].key,
        market_instance_id,
        market_binding.stored_bump,
    ) || !verifier.verify_market_runtime(
        base_program,
        accounts[IX_MARKET_RUNTIME].key,
        accounts[IX_MARKET_BINDING].key,
        market_runtime.stored_bump,
    ) || !verifier.verify_hoard_v2(
        base_program,
        accounts[IX_HOARD_V2].key,
        market_instance_id,
        hoard.stored_bump,
    ) || !verifier.verify_claim_ledger_v3(
        base_program,
        accounts[IX_CLAIM_LEDGER_V3].key,
        market_instance_id,
        claim_ledger.stored_bump,
    ) || !verifier.verify_product_artifact(
        base_program,
        accounts[IX_NATIVE_CLAIM_BASIS].key,
        32,
        basis_id,
    ) || !verifier.verify_market_instance_artifact(
        base_program,
        accounts[IX_MARKET_INSTANCE].key,
        market_instance_id,
    ) {
        return Err(Error::PdaMismatch);
    }

    let source = authenticate_position_v3(&accounts[IX_SOURCE_POSITION], base_program, verifier)?;
    let source_replay = authenticate_replay_v3(
        &accounts[IX_SOURCE_REPLAY],
        accounts[IX_SOURCE_POSITION].key,
        source,
        base_program,
        verifier,
        sha,
    )?;
    let destination =
        authenticate_position_v3(&accounts[IX_DESTINATION_POSITION], base_program, verifier)?;
    let destination_replay = authenticate_replay_v3(
        &accounts[IX_DESTINATION_REPLAY],
        accounts[IX_DESTINATION_POSITION].key,
        destination,
        base_program,
        verifier,
        sha,
    )?;
    validate_position_pair(
        source,
        &source_replay,
        &accounts[IX_SOURCE_POSITION],
        &accounts[IX_SOURCE_REPLAY],
        market_instance_id,
        collateral_realm.realm.bytes(),
        collateral_policy_id,
        collateral_release_id,
        basis.outcome_count,
        claim_ledger,
    )?;
    validate_position_pair(
        destination,
        &destination_replay,
        &accounts[IX_DESTINATION_POSITION],
        &accounts[IX_DESTINATION_REPLAY],
        market_instance_id,
        collateral_realm.realm.bytes(),
        collateral_policy_id,
        collateral_release_id,
        basis.outcome_count,
        claim_ledger,
    )?;

    let (local_action, user, vault) = direction(source, destination)?;
    let vault_authority = descriptor.addresses().vault_owner;
    if vault.owner().bytes() != vault_authority
        || vault.controller().bytes() != vault_authority
        || vault.purpose_binding_id().bytes() != descriptor.wrapper_product_id()
        || user.controller().bytes() != accounts[IX_ACTOR].key
        || user.purpose_binding_id().bytes() != accounts[IX_MARKET_RUNTIME].key
        || accounts[IX_VAULT_AUTHORITY].key != vault_authority
        || transfer.market != market_instance_id
        || transfer.source_owner != source.owner().bytes()
        || transfer.destination_owner != destination.owner().bytes()
        || transfer.source_generation != source.generation()
        || transfer.destination_generation != destination.generation()
        || transfer.source_replay_sequence != source_replay.header().next_sequence()
        || transfer.destination_replay_sequence != destination_replay.header().next_sequence()
        || transfer.authority_kind != PositionAssetTransferAuthorityKindV1::StructuredCustody
    {
        return Err(Error::CustodyAuthorityMismatch);
    }
    validate_transfer_padding(transfer, source.outcome_count())?;
    let transition = prepare_atomic_position_asset_transfer_v1(
        basis.outcome_count,
        market_phase,
        position_projection(source, &source_replay),
        position_projection(destination, &destination_replay),
        AtomicPositionAssetTransferRequestV1 {
            market: transfer.market,
            source_owner: transfer.source_owner,
            destination_owner: transfer.destination_owner,
            source_generation: transfer.source_generation,
            destination_generation: transfer.destination_generation,
            source_replay_sequence: transfer.source_replay_sequence,
            destination_replay_sequence: transfer.destination_replay_sequence,
            cash_atoms: transfer.cash_atoms,
            internal: transfer.internal,
            phase_policy: transfer.phase_policy,
        },
    )?;
    let source_post = position_successor(source, transition.source)?;
    let destination_post = position_successor(destination, transition.destination)?;

    let source_body_digest = source
        .semantic_id(&sha)
        .map_err(|_| Error::ProductBoundary)?
        .bytes();
    let destination_body_digest = destination
        .semantic_id(&sha)
        .map_err(|_| Error::ProductBoundary)?
        .bytes();
    let source_replay_semantic_id = source_replay
        .semantic_id(&sha)
        .map_err(|_| Error::ProductBoundary)?
        .bytes();
    let destination_replay_semantic_id = destination_replay
        .semantic_id(&sha)
        .map_err(|_| Error::ProductBoundary)?
        .bytes();
    let projection = StructuredCustodyCallProjectionV1 {
        target_base_program: base_program,
        wrapper_local_action: local_action,
        descriptor_account: accounts[IX_DESCRIPTOR].key,
        descriptor_body_digest: sha.hash(
            STRUCTURED_CUSTODY_DESCRIPTOR_BODY_DOMAIN_V1,
            accounts[IX_DESCRIPTOR].data,
        ),
        native_claim_id: descriptor.native_claim_id(),
        wrapper_product_id: descriptor.wrapper_product_id(),
        deployment: deployments.binding,
        hoard_account: accounts[IX_HOARD_V2].key,
        hoard_body_digest: sha.hash(
            STRUCTURED_CUSTODY_HOARD_BODY_DOMAIN_V1,
            accounts[IX_HOARD_V2].data,
        ),
        market_binding_account: accounts[IX_MARKET_BINDING].key,
        market_binding_body_digest: sha.hash(
            STRUCTURED_CUSTODY_MARKET_BINDING_BODY_DOMAIN_V1,
            accounts[IX_MARKET_BINDING].data,
        ),
        market_runtime_account: accounts[IX_MARKET_RUNTIME].key,
        market_runtime_body_digest: sha.hash(
            STRUCTURED_CUSTODY_MARKET_RUNTIME_BODY_DOMAIN_V1,
            accounts[IX_MARKET_RUNTIME].data,
        ),
        native_claim_basis_account: accounts[IX_NATIVE_CLAIM_BASIS].key,
        native_claim_basis_id: basis_id,
        market_instance_account: accounts[IX_MARKET_INSTANCE].key,
        market_instance_id,
        claim_ledger_account: accounts[IX_CLAIM_LEDGER_V3].key,
        claim_ledger_body_digest: sha.hash(
            STRUCTURED_CUSTODY_CLAIM_LEDGER_BODY_DOMAIN_V1,
            accounts[IX_CLAIM_LEDGER_V3].data,
        ),
        realm_id: collateral_realm.realm.bytes(),
        collateral_policy_id,
        collateral_release_id,
        vault_authority,
        user_actor: accounts[IX_ACTOR].key,
        source_position_account: accounts[IX_SOURCE_POSITION].key,
        source_position_body_digest: source_body_digest,
        source_replay_account: accounts[IX_SOURCE_REPLAY].key,
        source_replay_body_digest: source_replay_semantic_id,
        destination_position_account: accounts[IX_DESTINATION_POSITION].key,
        destination_position_body_digest: destination_body_digest,
        destination_replay_account: accounts[IX_DESTINATION_REPLAY].key,
        destination_replay_body_digest: destination_replay_semantic_id,
        transfer,
    };
    projection.encode_preimage_into(&mut scratch.authority_preimage)?;
    let authority_id = sha.hash(
        STRUCTURED_CUSTODY_CALL_V1_DOMAIN,
        &scratch.authority_preimage,
    );
    if is_zero(&authority_id)
        || (authenticate_final_digest && authority_id != transfer.authority_id)
        || (!authenticate_final_digest && transfer.authority_id != [0; 32])
    {
        return Err(Error::CustodyAuthorityMismatch);
    }
    let final_transfer = transfer.with_custody_authority(authority_id)?;
    let poststate = prepare_custody_poststate(
        accounts,
        descriptor,
        local_action,
        source,
        source_post,
        source_replay,
        source_body_digest,
        source_replay_semantic_id,
        destination,
        destination_post,
        destination_replay,
        destination_body_digest,
        destination_replay_semantic_id,
        authority_id,
        sha,
    )?;
    let cpi = build_cpi(base_program, accounts, final_transfer)?;
    Ok(AuthenticatedStructuredCustodyCallV1 {
        authority_id,
        local_action,
        transfer: final_transfer,
        source_after: transition.source,
        destination_after: transition.destination,
        poststate,
        cpi,
    })
}

#[allow(clippy::too_many_arguments)]
fn prepare_custody_poststate(
    accounts: &[RawAccountV1<'_>],
    descriptor: &BoundDescriptorV1,
    local_action: StructuredClaimActionV1,
    source: PositionAccountV3,
    source_post: PositionAccountV3,
    source_replay: ReplayV3Envelope<'_>,
    source_position_pre_id: Key,
    source_replay_pre_id: Key,
    destination: PositionAccountV3,
    destination_post: PositionAccountV3,
    destination_replay: ReplayV3Envelope<'_>,
    destination_position_pre_id: Key,
    destination_replay_pre_id: Key,
    transition_id: Key,
    sha: AdapterSha256V1,
) -> Result<StructuredCustodyPoststateV1> {
    let source_position = position_write(
        accounts[IX_SOURCE_POSITION].key,
        source_position_pre_id,
        source_post,
        sha,
    )?;
    let destination_position = position_write(
        accounts[IX_DESTINATION_POSITION].key,
        destination_position_pre_id,
        destination_post,
        sha,
    )?;
    let delta = StructuredClaimReplayDeltaV1 {
        action: local_action,
        source_sequence: source_replay.header().next_sequence(),
        destination_sequence: destination_replay.header().next_sequence(),
        transition_id,
        source_position_account: accounts[IX_SOURCE_POSITION].key,
        source_position_pre_semantic_id: source_position.prestate_semantic_id,
        source_position_post_semantic_id: source_position.poststate_semantic_id,
        destination_position_account: accounts[IX_DESTINATION_POSITION].key,
        destination_position_pre_semantic_id: destination_position.prestate_semantic_id,
        destination_position_post_semantic_id: destination_position.poststate_semantic_id,
    };
    let structured_delta_id = sha.hash(STRUCTURED_CLAIM_REPLAY_DELTA_DOMAIN_V1, &delta.encode()?);
    if is_zero(&structured_delta_id) || structured_delta_id == transition_id {
        return Err(Error::CustodyAuthorityMismatch);
    }

    let (general_replay, structured_replay, general_delta_id) =
        if source.purpose() == PositionPurposeV3::General {
            let (general, general_delta_id) = prepare_general_replay_write(
                &accounts[IX_SOURCE_POSITION],
                &accounts[IX_SOURCE_REPLAY],
                source,
                source_post,
                source_replay,
                source_position,
                source_replay_pre_id,
                accounts[IX_MARKET_RUNTIME].key,
                transition_id,
                structured_delta_id,
                sha,
            )?;
            let structured = prepare_structured_replay_write(
                &accounts[IX_DESTINATION_POSITION],
                &accounts[IX_DESTINATION_REPLAY],
                destination,
                destination_post,
                destination_replay,
                destination_position,
                destination_replay_pre_id,
                descriptor,
                local_action,
                transition_id,
                structured_delta_id,
                sha,
            )?;
            (general, structured, general_delta_id)
        } else {
            let structured = prepare_structured_replay_write(
                &accounts[IX_SOURCE_POSITION],
                &accounts[IX_SOURCE_REPLAY],
                source,
                source_post,
                source_replay,
                source_position,
                source_replay_pre_id,
                descriptor,
                local_action,
                transition_id,
                structured_delta_id,
                sha,
            )?;
            let (general, general_delta_id) = prepare_general_replay_write(
                &accounts[IX_DESTINATION_POSITION],
                &accounts[IX_DESTINATION_REPLAY],
                destination,
                destination_post,
                destination_replay,
                destination_position,
                destination_replay_pre_id,
                accounts[IX_MARKET_RUNTIME].key,
                transition_id,
                structured_delta_id,
                sha,
            )?;
            (general, structured, general_delta_id)
        };
    let (source_replay, destination_replay) = if source.purpose() == PositionPurposeV3::General {
        (general_replay, structured_replay)
    } else {
        (structured_replay, general_replay)
    };
    Ok(StructuredCustodyPoststateV1 {
        source_position,
        source_replay,
        destination_position,
        destination_replay,
        structured_delta_id,
        general_delta_id,
    })
}

#[allow(clippy::too_many_arguments)]
fn prepare_general_replay_write(
    position_account: &RawAccountV1<'_>,
    replay_account: &RawAccountV1<'_>,
    position: PositionAccountV3,
    position_post: PositionAccountV3,
    replay: ReplayV3Envelope<'_>,
    position_write: PositionV3WriteV1,
    replay_prestate_semantic_id: Key,
    general_market_runtime: Key,
    transition_id: Key,
    transition_evidence_id: Key,
    sha: AdapterSha256V1,
) -> Result<(ReplayV3WriteV1, Key)> {
    let authenticated_position = AuthenticatedPositionV3 {
        account: position_account.key,
        general_market_runtime,
        semantic: position,
        semantic_id: position_write.prestate_semantic_id,
        account_authenticated: true,
        semantic_id_authenticated: true,
        market_binding_authenticated: true,
        writable: position_account.writable,
    };
    let prestate = project_general_position_replay_prestate_v1(
        general_id(replay_account.key)?,
        replay.header().stored_bump(),
        replay.header().next_sequence(),
        replay_account.data,
        authenticated_position,
        &sha,
    )
    .map_err(|_| Error::CustodyAuthorityMismatch)?;
    let plan = project_general_replay_transition_v1(
        prestate,
        PositionSettlementPoststateV3 {
            account: position_account.key,
            general_market_runtime,
            prestate_semantic_id: position_write.prestate_semantic_id,
            semantic: position_post,
        },
        GeneralReplayTransitionKindV1::StructuredGeneral,
        general_id(transition_id)?,
        general_id(transition_evidence_id)?,
        &sha,
    )
    .map_err(|_| Error::CustodyAuthorityMismatch)?;
    if replay.header().extension_schema().get() != GENERAL_REPLAY_EXTENSION_SCHEMA_V1
        || plan.replay_prestate_semantic_id().bytes() != replay_prestate_semantic_id
        || plan.position_account().bytes() != position_account.key
        || plan.position_prestate_semantic_id().bytes() != position_write.prestate_semantic_id
        || plan.position_poststate_semantic_id().bytes() != position_write.poststate_semantic_id
        || plan.kind() != GeneralReplayTransitionKindV1::StructuredGeneral
        || plan.transition_id().bytes() != transition_id
        || plan.transition_evidence_id().bytes() != transition_evidence_id
        || plan.consumed_sequence() != replay.header().next_sequence()
        || plan.next_sequence()
            != replay
                .header()
                .next_sequence()
                .checked_add(1)
                .ok_or(Error::Arithmetic)?
    {
        return Err(Error::CustodyAuthorityMismatch);
    }
    let mut body = [0_u8; MAX_CUSTODY_REPLAY_V3_WRITE_BYTES];
    body[..GENERAL_REPLAY_ACCOUNT_V1_BYTES].copy_from_slice(plan.replay_poststate_body());
    let body_len = u16::try_from(GENERAL_REPLAY_ACCOUNT_V1_BYTES)
        .map_err(|_| Error::CustodyAuthorityMismatch)?;
    Ok((
        ReplayV3WriteV1 {
            address: plan.replay_account().bytes(),
            prestate_semantic_id: plan.replay_prestate_semantic_id().bytes(),
            poststate_semantic_id: plan.replay_poststate_semantic_id().bytes(),
            body_len,
            body,
        },
        plan.delta_id().bytes(),
    ))
}

#[allow(clippy::too_many_arguments)]
fn prepare_structured_replay_write(
    position_account: &RawAccountV1<'_>,
    replay_account: &RawAccountV1<'_>,
    position: PositionAccountV3,
    position_post: PositionAccountV3,
    replay: ReplayV3Envelope<'_>,
    position_write: PositionV3WriteV1,
    replay_prestate_semantic_id: Key,
    descriptor: &BoundDescriptorV1,
    action: StructuredClaimActionV1,
    transition_id: Key,
    delta_id: Key,
    sha: AdapterSha256V1,
) -> Result<ReplayV3WriteV1> {
    let header = replay.header();
    if header.extension_schema().get() != STRUCTURED_CLAIM_REPLAY_EXTENSION_SCHEMA_V1
        || header.position_account().bytes() != position_account.key
        || header.replay_account().bytes() != replay_account.key
        || header.purpose() != PositionPurposeV3::StructuredClaim
        || position.purpose() != PositionPurposeV3::StructuredClaim
        || position_post.purpose() != PositionPurposeV3::StructuredClaim
    {
        return Err(Error::CustodyAuthorityMismatch);
    }
    let extension = StructuredClaimReplayExtensionV1::decode(replay.extension())?;
    let extension_post = extension.advanced(StructuredClaimReplayTransitionV1 {
        descriptor_account: descriptor.addresses().descriptor,
        wrapper_product_id: descriptor.wrapper_product_id(),
        vault_authority: descriptor.addresses().vault_owner,
        action,
        transition_id,
        delta_id,
        position_pre_semantic_id: position_write.prestate_semantic_id,
        position_post_semantic_id: position_write.poststate_semantic_id,
    })?;
    let extension_body = extension_post.encode()?;
    let header_post = header
        .advanced_live(position_post.generation(), &extension_body, &sha)
        .map_err(|_| Error::CustodyAuthorityMismatch)?;
    let envelope_post = ReplayV3Envelope::from_header(header_post, &extension_body, &sha)
        .map_err(|_| Error::CustodyAuthorityMismatch)?;
    let mut body = [0_u8; MAX_CUSTODY_REPLAY_V3_WRITE_BYTES];
    envelope_post
        .encode_into(&mut body, &sha)
        .map_err(|_| Error::CustodyAuthorityMismatch)?;
    let poststate_semantic_id = envelope_post
        .semantic_id(&sha)
        .map_err(|_| Error::CustodyAuthorityMismatch)?
        .bytes();
    let body_len = u16::try_from(MAX_CUSTODY_REPLAY_V3_WRITE_BYTES)
        .map_err(|_| Error::CustodyAuthorityMismatch)?;
    Ok(ReplayV3WriteV1 {
        address: replay_account.key,
        prestate_semantic_id: replay_prestate_semantic_id,
        poststate_semantic_id,
        body_len,
        body,
    })
}

fn position_successor(
    prestate: PositionAccountV3,
    projection: PositionProjectionV1,
) -> Result<PositionAccountV3> {
    if projection.market != prestate.market_instance_id().bytes()
        || projection.owner != prestate.owner().bytes()
        || projection.generation != prestate.generation()
        || projection.reserved_cash_atoms != prestate.reserved_cash_atoms()
        || projection.closed
    {
        return Err(Error::PostStateMismatch);
    }
    let mut fields = prestate.fields();
    fields.cash_atoms = projection.cash_atoms;
    fields.reserved_cash_atoms = projection.reserved_cash_atoms;
    fields.native_eggs = projection.internal;
    PositionAccountV3::new(fields).map_err(|_| Error::PostStateMismatch)
}

fn position_write(
    address: Key,
    prestate_semantic_id: Key,
    poststate: PositionAccountV3,
    sha: AdapterSha256V1,
) -> Result<PositionV3WriteV1> {
    let body = poststate.encode().map_err(|_| Error::PostStateMismatch)?;
    let poststate_semantic_id = poststate
        .semantic_id(&sha)
        .map_err(|_| Error::PostStateMismatch)?
        .bytes();
    if is_zero(&poststate_semantic_id) || prestate_semantic_id == poststate_semantic_id {
        return Err(Error::PostStateMismatch);
    }
    Ok(PositionV3WriteV1 {
        address,
        prestate_semantic_id,
        poststate_semantic_id,
        body,
    })
}

fn validate_account_frame(
    accounts: &[RawAccountV1<'_>],
    descriptor: &BoundDescriptorV1,
    deployments: RuntimeDeploymentsV1,
) -> Result<()> {
    if accounts.len() != STRUCTURED_CUSTODY_ACCOUNT_COUNT {
        return Err(Error::InvalidAccounts);
    }
    deployments.validate()?;
    let expected_roles = [
        AccountRoleV1::VaultAuthority,
        AccountRoleV1::Realm,
        AccountRoleV1::Profile,
        AccountRoleV1::CollateralPolicy,
        AccountRoleV1::CollateralTokenProgram,
        AccountRoleV1::MarketBinding,
        AccountRoleV1::MarketRuntime,
        AccountRoleV1::SourcePositionV3,
        AccountRoleV1::SourceReplayV3,
        AccountRoleV1::DestinationPositionV3,
        AccountRoleV1::DestinationReplayV3,
        AccountRoleV1::Actor,
        AccountRoleV1::Descriptor,
        AccountRoleV1::WrapperProgram,
        AccountRoleV1::WrapperProgramData,
        AccountRoleV1::BaseProgram,
        AccountRoleV1::BaseProgramData,
        AccountRoleV1::Token2022Program,
        AccountRoleV1::Token2022ProgramData,
        AccountRoleV1::NativeClaimBasisArtifact,
        AccountRoleV1::MarketInstanceArtifact,
        AccountRoleV1::HoardV2,
        AccountRoleV1::ClaimLedgerV3,
    ];
    let writable = [
        false, false, false, false, false, false, false, true, true, true, true, false, false,
        false, false, false, false, false, false, false, false, false, false,
    ];
    let signer = [
        true, false, false, false, false, false, false, false, false, false, false, true, false,
        false, false, false, false, false, false, false, false, false, false,
    ];
    let executable = [
        false, false, false, false, true, false, false, false, false, false, false, false, false,
        true, false, true, false, true, false, false, false, false, false,
    ];
    let mut index = 0_usize;
    while index < accounts.len() {
        if accounts[index].role != expected_roles[index]
            || accounts[index].writable != writable[index]
            || accounts[index].signer != signer[index]
            || accounts[index].executable != executable[index]
            || is_zero(&accounts[index].key)
        {
            return Err(Error::InvalidAccounts);
        }
        let mut later = index + 1;
        while later < accounts.len() {
            if accounts[index].key == accounts[later].key
                && !is_admitted_readonly_program_alias(index, later)
            {
                return Err(Error::InvalidAccounts);
            }
            later += 1;
        }
        index += 1;
    }
    let binding = deployments.binding;
    if accounts[IX_DESCRIPTOR].key != descriptor.addresses().descriptor
        || accounts[IX_DESCRIPTOR].owner != binding.wrapper_program
        || accounts[IX_WRAPPER_PROGRAM].key != binding.wrapper_program
        || accounts[IX_WRAPPER_PROGRAM].owner != deployments.upgradeable_loader
        || accounts[IX_WRAPPER_PROGRAM_DATA].key != binding.wrapper_program_data
        || accounts[IX_WRAPPER_PROGRAM_DATA].owner != deployments.upgradeable_loader
        || accounts[IX_BASE_PROGRAM].key != binding.base_program
        || accounts[IX_BASE_PROGRAM].owner != deployments.upgradeable_loader
        || accounts[IX_BASE_PROGRAM_DATA].key != binding.base_program_data
        || accounts[IX_BASE_PROGRAM_DATA].owner != deployments.upgradeable_loader
        || accounts[IX_WRAPPER_TOKEN_2022_PROGRAM].key != binding.token_2022_program
        || accounts[IX_WRAPPER_TOKEN_2022_PROGRAM].owner != deployments.upgradeable_loader
        || accounts[IX_WRAPPER_TOKEN_2022_PROGRAM_DATA].key != binding.token_2022_program_data
        || accounts[IX_WRAPPER_TOKEN_2022_PROGRAM_DATA].owner != deployments.upgradeable_loader
    {
        return Err(Error::InvalidDeployment);
    }
    for index in [
        IX_REALM,
        IX_PROFILE,
        IX_COLLATERAL_POLICY,
        IX_MARKET_BINDING,
        IX_MARKET_RUNTIME,
        IX_SOURCE_POSITION,
        IX_SOURCE_REPLAY,
        IX_DESTINATION_POSITION,
        IX_DESTINATION_REPLAY,
        IX_NATIVE_CLAIM_BASIS,
        IX_MARKET_INSTANCE,
        IX_HOARD_V2,
        IX_CLAIM_LEDGER_V3,
    ] {
        if accounts[index].owner != binding.base_program {
            return Err(Error::InvalidAccounts);
        }
    }
    Ok(())
}

/// A Realm may select Token-2022 as its collateral program. In that case the
/// collateral executable and the wrapper-token executable are the same
/// read-only account. No data-bearing, signer, writable, ProgramData, or
/// semantically distinct role is permitted to alias.
const fn is_admitted_readonly_program_alias(left: usize, right: usize) -> bool {
    left == IX_COLLATERAL_TOKEN_PROGRAM && right == IX_WRAPPER_TOKEN_2022_PROGRAM
}

#[allow(clippy::too_many_arguments)]
fn validate_position_pair(
    position: PositionAccountV3,
    replay: &ReplayV3Envelope<'_>,
    position_account: &RawAccountV1<'_>,
    replay_account: &RawAccountV1<'_>,
    market_instance_id: Key,
    realm_id: Key,
    collateral_policy_id: Key,
    collateral_release_id: Key,
    outcome_count: u8,
    claim_ledger: ClaimLedgerV3,
) -> Result<()> {
    let replay_header = replay.header();
    if position.lifecycle() != PositionLifecycleV3::Open
        || position.market_instance_id().bytes() != market_instance_id
        || position.realm_id().bytes() != realm_id
        || position.collateral_policy_id().bytes() != collateral_policy_id
        || position.collateral_release_id().bytes() != collateral_release_id
        || position.outcome_count() != outcome_count
        || position.replay_account().bytes() != replay_account.key
        || replay_header.lifecycle() != ReplayV3Lifecycle::Live
        || replay_header.position_account().bytes() != position_account.key
        || replay_header.replay_account().bytes() != replay_account.key
        || replay_header.purpose() != position.purpose()
        || replay_header.purpose_binding_id() != position.purpose_binding_id()
        || replay_header.position_generation() != position.generation()
    {
        return Err(Error::PdaMismatch);
    }
    let internal = position.native_eggs();
    let mut outcome = 0_usize;
    while outcome < usize::from(outcome_count) {
        if internal[outcome] > claim_ledger.aggregate_internal_supply[outcome] {
            return Err(Error::BaseClosureMismatch);
        }
        outcome += 1;
    }
    Ok(())
}

fn authenticate_position_v3<P: StructuredCustodyPdaVerifierV1>(
    account: &RawAccountV1<'_>,
    base_program: Key,
    verifier: &P,
) -> Result<PositionAccountV3> {
    let position = PositionAccountV3::decode(account.data).map_err(|_| Error::ProductBoundary)?;
    if !verifier.verify_position_v3(base_program, account.key, position.pda_seeds()) {
        return Err(Error::PdaMismatch);
    }
    let _authenticated = authenticate_position_v3_exact(
        RetirementAccountViewV2 {
            address: identity(account.key)?,
            owner: identity(account.owner)?,
            data: account.data,
            is_writable: account.writable,
            is_executable: account.executable,
        },
        identity(base_program)?,
        CanonicalPdaV1::after_derivation(identity(account.key)?, position.stored_bump()),
        RetirementAccountAccessV2::Writable,
    )
    .map_err(|_| Error::InvalidAccounts)?;
    Ok(position)
}

fn authenticate_replay_v3<'a, P: StructuredCustodyPdaVerifierV1>(
    account: &RawAccountV1<'a>,
    position_account: Key,
    position: PositionAccountV3,
    base_program: Key,
    verifier: &P,
    sha: AdapterSha256V1,
) -> Result<ReplayV3Envelope<'a>> {
    let stored_bump = *account.data.get(4).ok_or(Error::ProductBoundary)?;
    if !verifier.verify_replay_v3(
        base_program,
        account.key,
        position_account,
        position.purpose(),
        position.purpose_binding_id().bytes(),
        stored_bump,
    ) {
        return Err(Error::PdaMismatch);
    }
    let authenticated = authenticate_purpose_replay_v3_exact(
        RetirementAccountViewV2 {
            address: identity(account.key)?,
            owner: identity(account.owner)?,
            data: account.data,
            is_writable: account.writable,
            is_executable: account.executable,
        },
        identity(base_program)?,
        CanonicalPdaV1::after_derivation(identity(account.key)?, stored_bump),
        RetirementAccountAccessV2::Writable,
    )
    .map_err(|_| Error::InvalidAccounts)?;
    ReplayV3Envelope::decode(authenticated.data(), &sha).map_err(|_| Error::ProductBoundary)
}

fn identity(bytes: Key) -> Result<Identity32V1> {
    Identity32V1::new(bytes).map_err(|_| Error::InvalidAccounts)
}

fn general_id(bytes: Key) -> Result<Id32> {
    Id32::new(bytes).map_err(|_| Error::CustodyAuthorityMismatch)
}

fn direction(
    source: PositionAccountV3,
    destination: PositionAccountV3,
) -> Result<(
    StructuredClaimActionV1,
    PositionAccountV3,
    PositionAccountV3,
)> {
    match (source.purpose(), destination.purpose()) {
        (PositionPurposeV3::General, PositionPurposeV3::StructuredClaim) => {
            Ok((StructuredClaimActionV1::WrapCanonical, source, destination))
        }
        (PositionPurposeV3::StructuredClaim, PositionPurposeV3::General) => Ok((
            StructuredClaimActionV1::UnwrapCanonical,
            destination,
            source,
        )),
        _ => Err(Error::CustodyAuthorityMismatch),
    }
}

fn validate_transfer_padding(
    transfer: PositionAssetTransferPayloadV1,
    outcome_count: u8,
) -> Result<()> {
    let width = usize::from(outcome_count);
    if !(2..=crate::runtime_contract::MAX_OUTCOMES).contains(&width) {
        return Err(Error::ProductBoundary);
    }
    let mut index = width;
    while index < crate::runtime_contract::MAX_OUTCOMES {
        if transfer.internal[index] != 0 {
            return Err(Error::CustodyAuthorityMismatch);
        }
        index += 1;
    }
    match transfer.phase_policy {
        AssetTransferPhasePolicyV1::ActiveOnly | AssetTransferPhasePolicyV1::ActiveOrResolved => {
            Ok(())
        }
    }
}

fn decode_market_phase(lifecycle: MarketLiabilityLifecycleV1) -> Result<MarketPhase> {
    match lifecycle {
        MarketLiabilityLifecycleV1::Open => Ok(MarketPhase::Active),
        MarketLiabilityLifecycleV1::Resolved => Ok(MarketPhase::Resolved),
        MarketLiabilityLifecycleV1::Retiring => Err(Error::ProductBoundary),
    }
}

fn position_projection(
    position: PositionAccountV3,
    replay: &ReplayV3Envelope<'_>,
) -> PositionProjectionV1 {
    PositionProjectionV1 {
        market: position.market_instance_id().bytes(),
        owner: position.owner().bytes(),
        generation: position.generation(),
        replay_sequence: replay.header().next_sequence(),
        cash_atoms: position.cash_atoms(),
        reserved_cash_atoms: position.reserved_cash_atoms(),
        internal: position.native_eggs(),
        closed: position.lifecycle() != PositionLifecycleV3::Open,
    }
}

fn build_cpi(
    base_program: Key,
    accounts: &[RawAccountV1<'_>],
    transfer: PositionAssetTransferPayloadV1,
) -> Result<BasePositionTransferCpiV1> {
    if accounts.len() != STRUCTURED_CUSTODY_ACCOUNT_COUNT {
        return Err(Error::InvalidAccounts);
    }
    let mut data = [0_u8; BASE_POSITION_TRANSFER_CPI_BYTES];
    let payload = transfer.encode()?;
    let request = ExtensionRequest {
        sequence: 0,
        envelope: ExtensionEnvelope {
            family: ExtensionFamily::GeneralV2,
            action: ExtensionAction::GeneralV2(GeneralV2Action::TransferPositionAssets),
            payload: &payload,
        },
    };
    let written = request
        .encode(&mut data)
        .map_err(|_| Error::InvalidInstruction)?;
    if written != BASE_POSITION_TRANSFER_CPI_BYTES {
        return Err(Error::InvalidInstruction);
    }
    let mut metas = [CpiAccountMetaV1::EMPTY; STRUCTURED_CUSTODY_ACCOUNT_COUNT];
    let mut index = 0_usize;
    while index < accounts.len() {
        metas[index] = CpiAccountMetaV1 {
            address: accounts[index].key,
            signer: accounts[index].signer,
            writable: accounts[index].writable,
        };
        index += 1;
    }
    Ok(BasePositionTransferCpiV1 {
        program_id: base_program,
        data,
        accounts: metas,
    })
}

const _: () = assert!(BASE_POSITION_TRANSFER_CPI_BYTES == 13 + 3 + 298);
const _: () = assert!(STRUCTURED_CUSTODY_ACCOUNT_COUNT == 23);

#[cfg(test)]
mod tests {
    use super::*;

    fn transfer() -> PositionAssetTransferPayloadV1 {
        let mut internal = [0; crate::runtime_contract::MAX_OUTCOMES];
        internal[0] = 1;
        PositionAssetTransferPayloadV1 {
            market: [1; 32],
            source_owner: [2; 32],
            destination_owner: [3; 32],
            source_generation: 4,
            destination_generation: 5,
            source_replay_sequence: 6,
            destination_replay_sequence: 7,
            cash_atoms: 0,
            internal,
            phase_policy: AssetTransferPhasePolicyV1::ActiveOnly,
            authority_kind: PositionAssetTransferAuthorityKindV1::StructuredCustody,
            authority_id: [8; 32],
        }
    }

    fn raw_accounts() -> [RawAccountV1<'static>; STRUCTURED_CUSTODY_ACCOUNT_COUNT] {
        let roles = [
            AccountRoleV1::VaultAuthority,
            AccountRoleV1::Realm,
            AccountRoleV1::Profile,
            AccountRoleV1::CollateralPolicy,
            AccountRoleV1::CollateralTokenProgram,
            AccountRoleV1::MarketBinding,
            AccountRoleV1::MarketRuntime,
            AccountRoleV1::SourcePositionV3,
            AccountRoleV1::SourceReplayV3,
            AccountRoleV1::DestinationPositionV3,
            AccountRoleV1::DestinationReplayV3,
            AccountRoleV1::Actor,
            AccountRoleV1::Descriptor,
            AccountRoleV1::WrapperProgram,
            AccountRoleV1::WrapperProgramData,
            AccountRoleV1::BaseProgram,
            AccountRoleV1::BaseProgramData,
            AccountRoleV1::Token2022Program,
            AccountRoleV1::Token2022ProgramData,
            AccountRoleV1::NativeClaimBasisArtifact,
            AccountRoleV1::MarketInstanceArtifact,
            AccountRoleV1::HoardV2,
            AccountRoleV1::ClaimLedgerV3,
        ];
        let mut accounts = [RawAccountV1 {
            role: AccountRoleV1::VaultAuthority,
            key: [1; 32],
            owner: [200; 32],
            lamports: 1,
            data: &[],
            signer: false,
            writable: false,
            executable: false,
        }; STRUCTURED_CUSTODY_ACCOUNT_COUNT];
        let mut index = 0_usize;
        while index < accounts.len() {
            accounts[index].role = roles[index];
            accounts[index].key = [u8::try_from(index + 1).expect("bounded account index"); 32];
            accounts[index].signer = index == IX_VAULT_AUTHORITY || index == IX_ACTOR;
            accounts[index].writable =
                (IX_SOURCE_POSITION..=IX_DESTINATION_REPLAY).contains(&index);
            accounts[index].executable = matches!(
                index,
                IX_COLLATERAL_TOKEN_PROGRAM
                    | IX_WRAPPER_PROGRAM
                    | IX_BASE_PROGRAM
                    | IX_WRAPPER_TOKEN_2022_PROGRAM
            );
            index += 1;
        }
        accounts
    }

    #[test]
    fn custody_cpi_uses_only_the_canonical_outer_extension_request() {
        let accounts = raw_accounts();
        let plan = build_cpi([200; 32], &accounts, transfer()).unwrap();
        let decoded = ExtensionRequest::decode(&plan.data).unwrap();
        assert_eq!(decoded.sequence, 0);
        assert_eq!(
            decoded.envelope.action,
            ExtensionAction::GeneralV2(GeneralV2Action::TransferPositionAssets)
        );
        assert_eq!(decoded.envelope.payload, &transfer().encode().unwrap());
        assert!(ExtensionRequest::decode(&plan.data[13..]).is_err());
    }

    #[test]
    fn custody_cpi_refuses_truncated_or_expanded_account_contracts() {
        let accounts = raw_accounts();
        assert_eq!(
            build_cpi([200; 32], &accounts[..accounts.len() - 1], transfer()),
            Err(Error::InvalidAccounts)
        );
        assert_eq!(
            build_cpi([200; 32], &[], transfer()),
            Err(Error::InvalidAccounts)
        );
    }

    #[test]
    fn account_frame_allows_only_the_token_2022_collateral_executable_alias() {
        let mut accounts = raw_accounts();
        accounts[IX_COLLATERAL_TOKEN_PROGRAM].key =
            accounts[IX_WRAPPER_TOKEN_2022_PROGRAM].key;
        assert!(is_admitted_readonly_program_alias(
            IX_COLLATERAL_TOKEN_PROGRAM,
            IX_WRAPPER_TOKEN_2022_PROGRAM
        ));
        accounts[IX_REALM].key = accounts[IX_PROFILE].key;
        assert!(!is_admitted_readonly_program_alias(IX_REALM, IX_PROFILE));
    }
}
