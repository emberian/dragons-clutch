//! Exact current Position V3 and purpose-Replay V3 successor plans.

use clutch_collateral_adapter_v2::{ClaimLedgerV3, HoardV2};
use clutch_general_v2_contract::{
    project_general_position_replay_prestate_v1, project_general_replay_transition_v1,
    GeneralReplayTransitionKindV1, Id32, GENERAL_REPLAY_ACCOUNT_V1_BYTES,
    GENERAL_REPLAY_EXTENSION_SCHEMA_V1,
};
use clutch_owner_settlement::{AuthenticatedPositionV3, PositionSettlementPoststateV3};
use clutch_retirement::{
    Identity32V1, PositionAccountV3, PositionLifecycleV3, PositionPurposeV3,
    PositionV3Sha256Backend, ReplayV3Envelope, ReplayV3HashBackend, ReplayV3Lifecycle,
};
use clutch_retirement_adapter::{
    authenticate_position_v3_exact, authenticate_purpose_replay_v3_exact,
    AccountAccessV2 as RetirementAccountAccessV2, AccountViewV2 as RetirementAccountViewV2,
    CanonicalPdaV1,
};
#[cfg(not(target_os = "solana"))]
use sha2::{Digest, Sha256};

use crate::runtime_contract::{
    PositionProjectionV1, StructuredClaimActionV1, StructuredClaimReplayDeltaV1,
    StructuredClaimReplayExtensionV1, StructuredClaimReplayTransitionV1,
    StructuredClaimVaultReplayDeltaV1, STRUCTURED_CLAIM_REPLAY_DELTA_DOMAIN_V1,
    STRUCTURED_CLAIM_REPLAY_EXTENSION_SCHEMA_V1,
    STRUCTURED_CLAIM_VAULT_REPLAY_DELTA_DOMAIN_V1,
};
use crate::{
    is_zero, AccountRoleV1, BasePositionPdaVerifierV1, BoundDescriptorV1,
    CurrentStructuredTransitionPlanV1, Error, Key, RawAccountV1, Result,
};

/// Digest domain for an exact canonical live descriptor body.
pub const STRUCTURED_CUSTODY_DESCRIPTOR_BODY_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-custody/descriptor-body/v1\0";
/// Exact core projection width shared by current full-vector actions.
pub const STRUCTURED_CUSTODY_ACCOUNT_COUNT: usize = 23;
const IX_MARKET_RUNTIME: usize = 6;
const IX_SOURCE_POSITION: usize = 7;
const IX_SOURCE_REPLAY: usize = 8;
const IX_DESTINATION_POSITION: usize = 9;
const IX_DESTINATION_REPLAY: usize = 10;
const IX_BASE_PROGRAM: usize = 15;
const IX_HOARD_V2: usize = 21;
const IX_CLAIM_LEDGER_V3: usize = 22;
/// Exact canonical Position V3 successor width staged by current actions.
pub const POSITION_V3_WRITE_BYTES: usize = 480;
/// Largest Replay V3 body staged by this bridge (common prefix + SCV1).
pub const MAX_CUSTODY_REPLAY_V3_WRITE_BYTES: usize = 416;

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

/// Atomic single-vault Position V3 and Replay V3 compaction successors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredVaultPoststateV1 {
    /// Structured-purpose vault Position successor.
    pub vault_position: PositionV3WriteV1,
    /// Structured-purpose vault Replay successor.
    pub vault_replay: ReplayV3WriteV1,
    /// Exact single-vault compaction delta identity.
    pub structured_delta_id: Key,
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

/// Build exact Position/Replay V3 postimages for a current full-width route.
///
/// The caller must have produced `plan` directly from the same authenticated
/// Hoard V2, ClaimLedger V3, Position, mint, and holder prestates. This helper
/// independently re-decodes both purpose-owned Position/Replay pairs, checks
/// the plan's owner semantic IDs against the presented bytes, and advances the
/// General and Structured replay owners exactly once around the plan receipt.
/// It performs no write and grants no CPI authority by itself.
pub fn prepare_current_structured_position_poststate_v1<
    P: BasePositionPdaVerifierV1,
>(
    accounts: &[RawAccountV1<'_>],
    descriptor: &BoundDescriptorV1,
    plan: CurrentStructuredTransitionPlanV1,
    verifier: &P,
) -> Result<StructuredCustodyPoststateV1> {
    if accounts.len() < STRUCTURED_CUSTODY_ACCOUNT_COUNT
        || !matches!(
            plan.action,
            StructuredClaimActionV1::WrapFull
                | StructuredClaimActionV1::UnwrapFull
                | StructuredClaimActionV1::RedeemTerminal
        )
        || is_zero(&plan.transition_id)
    {
        return Err(Error::CustodyAuthorityMismatch);
    }
    let base_program = descriptor.identity().deployment.base_program;
    if accounts[IX_BASE_PROGRAM].key != base_program {
        return Err(Error::CustodyAuthorityMismatch);
    }
    let sha = AdapterSha256V1;
    let hoard = HoardV2::decode(accounts[IX_HOARD_V2].data)
        .map_err(|_| Error::BaseClosureMismatch)?;
    let claim_ledger = ClaimLedgerV3::decode(accounts[IX_CLAIM_LEDGER_V3].data)
        .map_err(|_| Error::BaseClosureMismatch)?;
    if hoard
        .semantic_id(&sha)
        .map_err(|_| Error::BaseClosureMismatch)?
        .bytes()
        != plan.hoard_before_id
        || claim_ledger
            .semantic_id(&sha)
            .map_err(|_| Error::BaseClosureMismatch)?
            .bytes()
            != plan.claim_ledger_before_id
        || plan
            .hoard_after
            .semantic_id(&sha)
            .map_err(|_| Error::BaseClosureMismatch)?
            .bytes()
            != plan.hoard_after_id
        || plan
            .claim_ledger_after
            .semantic_id(&sha)
            .map_err(|_| Error::BaseClosureMismatch)?
            .bytes()
            != plan.claim_ledger_after_id
    {
        return Err(Error::BaseClosureMismatch);
    }

    let source = authenticate_position_v3(
        &accounts[IX_SOURCE_POSITION],
        base_program,
        verifier,
    )?;
    let source_replay = authenticate_replay_v3(
        &accounts[IX_SOURCE_REPLAY],
        accounts[IX_SOURCE_POSITION].key,
        source,
        base_program,
        verifier,
        sha,
    )?;
    let destination = authenticate_position_v3(
        &accounts[IX_DESTINATION_POSITION],
        base_program,
        verifier,
    )?;
    let destination_replay = authenticate_replay_v3(
        &accounts[IX_DESTINATION_REPLAY],
        accounts[IX_DESTINATION_POSITION].key,
        destination,
        base_program,
        verifier,
        sha,
    )?;
    for (position, replay, position_account, replay_account) in [
        (
            source,
            &source_replay,
            &accounts[IX_SOURCE_POSITION],
            &accounts[IX_SOURCE_REPLAY],
        ),
        (
            destination,
            &destination_replay,
            &accounts[IX_DESTINATION_POSITION],
            &accounts[IX_DESTINATION_REPLAY],
        ),
    ] {
        validate_position_pair(
            position,
            replay,
            position_account,
            replay_account,
            descriptor.identity().claim.basis.market,
            hoard.realm_id.bytes(),
            hoard.collateral_policy_id.bytes(),
            hoard.collateral_release_id.bytes(),
            claim_ledger.outcome_count,
            claim_ledger,
        )?;
    }
    let user_after = plan.user_after.ok_or(Error::PostStateMismatch)?;
    let (source_projection, destination_projection) = match plan.action {
        StructuredClaimActionV1::WrapFull => {
            if source.purpose() != PositionPurposeV3::General
                || destination.purpose() != PositionPurposeV3::StructuredClaim
            {
                return Err(Error::CustodyAuthorityMismatch);
            }
            (user_after, plan.vault_after)
        }
        StructuredClaimActionV1::UnwrapFull | StructuredClaimActionV1::RedeemTerminal => {
            if source.purpose() != PositionPurposeV3::StructuredClaim
                || destination.purpose() != PositionPurposeV3::General
            {
                return Err(Error::CustodyAuthorityMismatch);
            }
            (plan.vault_after, user_after)
        }
        _ => return Err(Error::CustodyAuthorityMismatch),
    };
    let source_post = position_successor(source, source_projection)?;
    let destination_post = position_successor(destination, destination_projection)?;
    let source_position_pre_id = source
        .semantic_id(&sha)
        .map_err(|_| Error::PostStateMismatch)?
        .bytes();
    let destination_position_pre_id = destination
        .semantic_id(&sha)
        .map_err(|_| Error::PostStateMismatch)?
        .bytes();
    let source_replay_pre_id = source_replay
        .semantic_id(&sha)
        .map_err(|_| Error::PostStateMismatch)?
        .bytes();
    let destination_replay_pre_id = destination_replay
        .semantic_id(&sha)
        .map_err(|_| Error::PostStateMismatch)?
        .bytes();
    prepare_custody_poststate(
        accounts,
        descriptor,
        plan.action,
        source,
        source_post,
        source_replay,
        source_position_pre_id,
        source_replay_pre_id,
        destination,
        destination_post,
        destination_replay,
        destination_position_pre_id,
        destination_replay_pre_id,
        plan.transition_id,
        sha,
    )
}

/// Build exact single-vault Position/Replay V3 postimages for compaction.
///
/// The caller presents the exact authenticated Hoard and ClaimLedger bodies
/// used by the current compaction planner. This helper independently hostile-
/// decodes the Structured pair, checks every immutable owner/rent coordinate,
/// and advances only its purpose Replay around the transition receipt.
pub fn prepare_current_structured_vault_poststate_v1<P: BasePositionPdaVerifierV1>(
    vault_position_account: &RawAccountV1<'_>,
    vault_replay_account: &RawAccountV1<'_>,
    hoard_account: &RawAccountV1<'_>,
    claim_ledger_account: &RawAccountV1<'_>,
    descriptor: &BoundDescriptorV1,
    plan: CurrentStructuredTransitionPlanV1,
    verifier: &P,
) -> Result<StructuredVaultPoststateV1> {
    if plan.action != StructuredClaimActionV1::CompactDonation
        || plan.user_after.is_some()
        || is_zero(&plan.transition_id)
        || vault_position_account.role != AccountRoleV1::SourcePositionV3
        || vault_replay_account.role != AccountRoleV1::SourceReplayV3
        || hoard_account.role != AccountRoleV1::HoardV2
        || claim_ledger_account.role != AccountRoleV1::ClaimLedgerV3
        || !vault_position_account.writable
        || !vault_replay_account.writable
        || !hoard_account.writable
        || !claim_ledger_account.writable
    {
        return Err(Error::CustodyAuthorityMismatch);
    }
    let base_program = descriptor.identity().deployment.base_program;
    for account in [
        vault_position_account,
        vault_replay_account,
        hoard_account,
        claim_ledger_account,
    ] {
        if account.owner != base_program || account.executable || is_zero(&account.key) {
            return Err(Error::InvalidAccounts);
        }
    }
    let keys = [
        vault_position_account.key,
        vault_replay_account.key,
        hoard_account.key,
        claim_ledger_account.key,
    ];
    let mut left = 0usize;
    while left < keys.len() {
        let mut right = left + 1;
        while right < keys.len() {
            if keys[left] == keys[right] {
                return Err(Error::InvalidAccounts);
            }
            right += 1;
        }
        left += 1;
    }

    let sha = AdapterSha256V1;
    let hoard = HoardV2::decode(hoard_account.data).map_err(|_| Error::BaseClosureMismatch)?;
    let claim_ledger = ClaimLedgerV3::decode(claim_ledger_account.data)
        .map_err(|_| Error::BaseClosureMismatch)?;
    if hoard
        .semantic_id(&sha)
        .map_err(|_| Error::BaseClosureMismatch)?
        .bytes()
        != plan.hoard_before_id
        || claim_ledger
            .semantic_id(&sha)
            .map_err(|_| Error::BaseClosureMismatch)?
            .bytes()
            != plan.claim_ledger_before_id
        || plan
            .hoard_after
            .semantic_id(&sha)
            .map_err(|_| Error::BaseClosureMismatch)?
            .bytes()
            != plan.hoard_after_id
        || plan
            .claim_ledger_after
            .semantic_id(&sha)
            .map_err(|_| Error::BaseClosureMismatch)?
            .bytes()
            != plan.claim_ledger_after_id
    {
        return Err(Error::BaseClosureMismatch);
    }
    let vault = authenticate_position_v3(vault_position_account, base_program, verifier)?;
    let replay = authenticate_replay_v3(
        vault_replay_account,
        vault_position_account.key,
        vault,
        base_program,
        verifier,
        sha,
    )?;
    validate_position_pair(
        vault,
        &replay,
        vault_position_account,
        vault_replay_account,
        descriptor.identity().claim.basis.market,
        hoard.realm_id.bytes(),
        hoard.collateral_policy_id.bytes(),
        hoard.collateral_release_id.bytes(),
        claim_ledger.outcome_count,
        claim_ledger,
    )?;
    let header = replay.header();
    if vault.purpose() != PositionPurposeV3::StructuredClaim
        || vault.owner().bytes() != descriptor.addresses().vault_owner
        || vault.purpose_binding_id().bytes() != descriptor.wrapper_product_id()
        || plan.vault_after.market != vault.market_instance_id().bytes()
        || plan.vault_after.owner != vault.owner().bytes()
        || plan.vault_after.generation != vault.generation()
        || plan.vault_after.replay_sequence
            != header
                .next_sequence()
                .checked_add(1)
                .ok_or(Error::Arithmetic)?
        || plan.vault_after.reserved_cash_atoms != vault.reserved_cash_atoms()
        || plan.vault_after.closed
    {
        return Err(Error::PostStateMismatch);
    }
    let vault_post = position_successor(vault, plan.vault_after)?;
    let vault_pre_id = vault
        .semantic_id(&sha)
        .map_err(|_| Error::PostStateMismatch)?
        .bytes();
    let replay_pre_id = replay
        .semantic_id(&sha)
        .map_err(|_| Error::PostStateMismatch)?
        .bytes();
    let vault_position = position_write(
        vault_position_account.key,
        vault_pre_id,
        vault_post,
        sha,
    )?;
    let delta = StructuredClaimVaultReplayDeltaV1 {
        action: StructuredClaimActionV1::CompactDonation,
        sequence: header.next_sequence(),
        transition_id: plan.transition_id,
        position_account: vault_position_account.key,
        position_pre_semantic_id: vault_position.prestate_semantic_id,
        position_post_semantic_id: vault_position.poststate_semantic_id,
    };
    let structured_delta_id = sha.hash(
        STRUCTURED_CLAIM_VAULT_REPLAY_DELTA_DOMAIN_V1,
        &delta.encode()?,
    );
    if is_zero(&structured_delta_id) || structured_delta_id == plan.transition_id {
        return Err(Error::CustodyAuthorityMismatch);
    }
    let vault_replay = prepare_structured_replay_write(
        vault_position_account,
        vault_replay_account,
        vault,
        vault_post,
        replay,
        vault_position,
        replay_pre_id,
        descriptor,
        StructuredClaimActionV1::CompactDonation,
        plan.transition_id,
        structured_delta_id,
        sha,
    )?;
    Ok(StructuredVaultPoststateV1 {
        vault_position,
        vault_replay,
        structured_delta_id,
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

fn authenticate_position_v3<P: BasePositionPdaVerifierV1>(
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

fn authenticate_replay_v3<'a, P: BasePositionPdaVerifierV1>(
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

const _: () = assert!(STRUCTURED_CUSTODY_ACCOUNT_COUNT == 23);
