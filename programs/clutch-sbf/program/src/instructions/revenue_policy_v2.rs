//! Realm-owned immutable fee-bearing revenue authority.
//!
//! Action 1 creates a Realm and RevenuePolicyRecordV2 in one rollback domain.
//! The caller supplies one complete canonical policy preimage; no treasury or
//! rate field exists beside it.  The current unified development profile
//! admits only the named 40/10-bp, 60/0/40 calibration.  The codec and private
//! authentication receipt remain policy-generic so a later checked release may
//! admit another registered immutable V2 member without changing Realm bytes.
//!
//! Action 2 is the permissionless rent close.  It is reachable economically
//! only after the Realm account is absent, returns exactly recorded principal
//! to the recorded payer, and sends the initial hostile prefund plus every
//! later surplus to the canonical neutral sink.

use crate::accounts::{
    self, expect_pda, require, require_count, require_distinct, require_signer, Outcome,
};
use crate::error::{ClutchError, Refusal};
use crate::{seeds, instructions::genesis};
use clutch_batch_policy_identity::revenue_policy_v2::{
    decode_revenue_policy_v2, revenue_policy_record_v2_id, revenue_policy_v2_digest,
    treasury_position_derivation_policy_v2_id, RevenuePolicyV2,
};
use clutch_general_v2_contract::{
    found_general_position_replay_v1, found_general_position_v1, found_general_replay_v1,
    GENERAL_POSITION_FOUNDING_GENERATION_V1, GENERAL_REPLAY_ACCOUNT_V1_BYTES,
};
use clutch_retirement::{
    DeletableRentOwnerV1, Identity32V1, PositionAccountV3, PositionPurposeV3,
    PositionV3Sha256Backend, RentSplitV2, ReplayV3Envelope, ReplayV3HashBackend,
    POSITION_TOMBSTONE_V3_BYTES, POSITION_V3_BYTES,
};
use clutch_solana_layout::registry::RealmRevenueV2Action;
use clutch_solana_layout::revenue::{
    CloseRevenuePolicyRecordV2Payload, InitializeFeeBearingRealmV2Payload,
    RevenuePolicyRecordV2, TreasuryServiceLedgerV1, REVENUE_POLICY_RECORD_BYTES_V2,
    TREASURY_SERVICE_LEDGER_V1_BYTES,
};
use clutch_solana_layout::{
    account_len, canonical_realm_id, Hash32, RealmAccount,
};
use clutch_product_series::{ContentId, MarketFoundationSlotV4};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;
use solana_sdk_ids::incinerator;

use super::direct_selection_v3::{
    create_pda_account_full_principal, direct_creation_funding, DIRECT_NEUTRAL_SINK_V3,
};
use super::general_market_foundation_v4::allocate_assign_product_funded_pda;
use super::product_market_foundation_current::AuthenticatedProductMarketFoundationDebitV4;

const TREASURY_SERVICE_ADMISSION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/treasury-service/admit/v1\0";
const TREASURY_SERVICE_SETTLEMENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/treasury-service/settle/v1\0";
const TREASURY_SERVICE_CLOSE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/treasury-service/close/v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeSha256;

impl PositionV3Sha256Backend for RuntimeSha256 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        solana_sha256_hasher::hashv(&[domain, body]).to_bytes()
    }
}

impl ReplayV3HashBackend for RuntimeSha256 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

/// Exact accounts for action 1.
pub const INITIALIZE_FEE_BEARING_REALM_V2_ACCOUNT_COUNT: usize = 6;
/// Founding payer: signer and writable.
pub const IX_REVENUE_V2_PAYER: usize = 0;
/// Absent canonical Realm PDA: writable.
pub const IX_REVENUE_V2_REALM: usize = 1;
/// Canonical sealed collateral policy: program-owned and read-only.
pub const IX_REVENUE_V2_COLLATERAL_POLICY: usize = 2;
/// Absent canonical RevenuePolicyRecordV2 PDA: writable.
pub const IX_REVENUE_V2_RECORD: usize = 3;
/// System program.
pub const IX_REVENUE_V2_SYSTEM: usize = 4;
/// Rent sysvar.
pub const IX_REVENUE_V2_RENT: usize = 5;

/// Exact accounts for action 2: absent Realm, record, refund owner, neutral
/// sink.  No signer is needed because destinations are immutable record facts.
pub const CLOSE_REVENUE_POLICY_RECORD_V2_ACCOUNT_COUNT: usize = 4;
/// Provably absent canonical Realm PDA.
pub const IX_CLOSE_REVENUE_V2_REALM: usize = 0;
/// Live V2 record, writable.
pub const IX_CLOSE_REVENUE_V2_RECORD: usize = 1;
/// Exact recorded payer, writable.
pub const IX_CLOSE_REVENUE_V2_PAYER: usize = 2;
/// Canonical neutral sink, writable.
pub const IX_CLOSE_REVENUE_V2_SINK: usize = 3;

/// Private, non-caller-constructible authentication of one live V2 record and
/// its exact policy preimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedRevenuePolicyRecordV2 {
    realm: Hash32,
    record_account: Pubkey,
    record_semantic_id: Hash32,
    policy_digest: Hash32,
    policy: RevenuePolicyV2,
    treasury_position_derivation_policy_id: Hash32,
}

impl AuthenticatedRevenuePolicyRecordV2 {
    /// Exact Realm identity.
    pub(crate) const fn realm(self) -> Hash32 {
        self.realm
    }

    /// Physical immutable record account.
    pub(crate) const fn record_account(self) -> Pubkey {
        self.record_account
    }

    /// Rent-independent semantic record identity.
    pub(crate) const fn record_semantic_id(self) -> Hash32 {
        self.record_semantic_id
    }

    /// Exact RevenuePolicyV2 digest.
    pub(crate) const fn policy_digest(self) -> Hash32 {
        self.policy_digest
    }

    /// Complete exact policy, including rates, split, and treasury owner.
    pub(crate) const fn policy(self) -> RevenuePolicyV2 {
        self.policy
    }

    /// Immutable Realm-selected treasury owner.
    pub(crate) const fn treasury_owner(self) -> Hash32 {
        Hash32::from_bytes(self.policy.treasury_owner)
    }

    /// Exact typed Market-scoped Position/service-ledger derivation policy ID.
    pub(crate) const fn treasury_position_derivation_policy_id(self) -> Hash32 {
        self.treasury_position_derivation_policy_id
    }
}

/// Private deterministic Market treasury coordinates derived from one
/// authenticated Realm record.  This is not a creation receipt: Product's
/// Market founder must still create and independently rent-fund both accounts,
/// authenticate their postimages, and only then mint its General authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RevenueMarketTreasuryDerivationV1 {
    authority: AuthenticatedRevenuePolicyRecordV2,
    market_instance_v2_id: Hash32,
    general_market_runtime_account: Pubkey,
    treasury_position_account: Pubkey,
    treasury_position_bump: u8,
    treasury_replay_account: Pubkey,
    treasury_replay_bump: u8,
    treasury_service_ledger_account: Pubkey,
    treasury_service_ledger_bump: u8,
}

impl RevenueMarketTreasuryDerivationV1 {
    /// Authenticated Realm revenue authority.
    pub(crate) const fn authority(self) -> AuthenticatedRevenuePolicyRecordV2 {
        self.authority
    }

    /// Full MarketInstanceV2 identity.
    pub(crate) const fn market_instance_v2_id(self) -> Hash32 {
        self.market_instance_v2_id
    }

    /// Canonical General MarketRuntime used as Position/Replay purpose binding.
    pub(crate) const fn general_market_runtime_account(self) -> Pubkey {
        self.general_market_runtime_account
    }

    /// Canonical ordinary treasury PositionV3 account.
    pub(crate) const fn treasury_position_account(self) -> Pubkey {
        self.treasury_position_account
    }

    /// Canonical ordinary treasury PositionV3 bump.
    pub(crate) const fn treasury_position_bump(self) -> u8 {
        self.treasury_position_bump
    }

    /// Canonical mandatory purpose-owned GEN1 ReplayV3 account.
    pub(crate) const fn treasury_replay_account(self) -> Pubkey {
        self.treasury_replay_account
    }

    /// Canonical mandatory purpose-owned GEN1 ReplayV3 bump.
    pub(crate) const fn treasury_replay_bump(self) -> u8 {
        self.treasury_replay_bump
    }

    /// Canonical counted treasury-service-ledger account.
    pub(crate) const fn treasury_service_ledger_account(self) -> Pubkey {
        self.treasury_service_ledger_account
    }

    /// Canonical counted treasury-service-ledger bump.
    pub(crate) const fn treasury_service_ledger_bump(self) -> u8 {
        self.treasury_service_ledger_bump
    }
}

/// Default-refusing Product/General Market facts required to recognize an
/// ordinary treasury Position/Replay founding postwrite.  The eventual
/// Product composer implements this only for its private authenticated
/// current-family authority; callers cannot supply a public DTO here.
pub(crate) trait AuthenticatedTreasuryMarketFactsV1 {
    /// Full MarketInstanceV2 identity.
    fn market_instance_v2_id(&self) -> Option<Hash32> {
        None
    }
    /// Realm identity.
    fn realm(&self) -> Option<Hash32> {
        None
    }
    /// Realm-selected collateral policy identity.
    fn collateral_policy_id(&self) -> Option<Hash32> {
        None
    }
    /// Exact compiled collateral release identity.
    fn collateral_release_id(&self) -> Option<Hash32> {
        None
    }
    /// Canonical General MarketRuntime account.
    fn general_market_runtime_account(&self) -> Option<Pubkey> {
        None
    }
    /// Active Market outcome width.
    fn outcome_count(&self) -> Option<u8> {
        None
    }
}

/// Exact actual Position/Replay founding postwrite.  Private fields ensure the
/// service ledger cannot be founded from expected addresses without comparing
/// both real account bodies and their rent compartments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedTreasuryPositionReplayFoundationV1 {
    derivation: RevenueMarketTreasuryDerivationV1,
    position_generation: u64,
    position_semantic_id: Hash32,
    replay_semantic_id: Hash32,
}

impl AuthenticatedTreasuryPositionReplayFoundationV1 {
    /// Exact Revenue/Market derivation.
    pub(crate) const fn derivation(self) -> RevenueMarketTreasuryDerivationV1 {
        self.derivation
    }

    /// Exact founding Position generation.
    pub(crate) const fn position_generation(self) -> u64 {
        self.position_generation
    }

    /// Semantic ID of the actual founding Position postimage.
    pub(crate) const fn position_semantic_id(self) -> Hash32 {
        self.position_semantic_id
    }

    /// Semantic ID of the actual founding Replay postimage.
    pub(crate) const fn replay_semantic_id(self) -> Hash32 {
        self.replay_semantic_id
    }
}

/// Move-only hostile postwrite for one Product-funded treasury foundation
/// slot. General wraps this with the exact Product debit before Product may
/// advance its ScheduleV4 cursor.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductFundedTreasurySlotV1 {
    slot: MarketFoundationSlotV4,
    account: Pubkey,
    account_data_id: ContentId,
    semantic_id: Hash32,
}

impl AuthenticatedProductFundedTreasurySlotV1 {
    pub(crate) const fn slot(&self) -> MarketFoundationSlotV4 { self.slot }
    pub(crate) const fn account(&self) -> Pubkey { self.account }
    pub(crate) const fn account_data_id(&self) -> ContentId { self.account_data_id }
    pub(crate) const fn semantic_id(&self) -> Hash32 { self.semantic_id }
}

/// Atomic Product/General founding result for one usable ordinary treasury
/// Position, its mandatory Replay, and its counted service ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RevenueMarketTreasuryFoundationV1 {
    position_replay: AuthenticatedTreasuryPositionReplayFoundationV1,
    service_ledger: AuthenticatedTreasuryServiceLedgerV1,
}

impl RevenueMarketTreasuryFoundationV1 {
    /// Exact authenticated Position/Replay postwrite.
    pub(crate) const fn position_replay(
        self,
    ) -> AuthenticatedTreasuryPositionReplayFoundationV1 {
        self.position_replay
    }

    /// Exact authenticated zero-count service-ledger postwrite.
    pub(crate) const fn service_ledger(self) -> AuthenticatedTreasuryServiceLedgerV1 {
        self.service_ledger
    }

    /// Exact Realm identity.
    pub(crate) const fn realm(self) -> Hash32 {
        self.position_replay.derivation.authority.realm
    }

    /// Physical immutable RevenuePolicyRecordV2 account.
    pub(crate) const fn revenue_policy_record_account(self) -> Pubkey {
        self.position_replay.derivation.authority.record_account
    }

    /// Rent-independent RevenuePolicyRecordV2 semantic identity.
    pub(crate) const fn revenue_policy_record_v2_id(self) -> Hash32 {
        self.position_replay.derivation.authority.record_semantic_id
    }

    /// Exact RevenuePolicyV2 digest.
    pub(crate) const fn revenue_policy_v2_digest(self) -> Hash32 {
        self.position_replay.derivation.authority.policy_digest
    }

    /// Immutable treasury beneficiary.
    pub(crate) const fn treasury_owner(self) -> Hash32 {
        self.position_replay.derivation.authority.treasury_owner()
    }

    /// Typed ordinary-Position derivation-policy identity.
    pub(crate) const fn treasury_position_derivation_policy_v2_id(self) -> Hash32 {
        self.position_replay
            .derivation
            .authority
            .treasury_position_derivation_policy_id
    }

    /// Canonical ordinary PositionV3 account.
    pub(crate) const fn treasury_position_account(self) -> Pubkey {
        self.position_replay.derivation.treasury_position_account
    }

    /// Mandatory deterministic GEN1 ReplayV3 account.
    pub(crate) const fn treasury_replay_account(self) -> Pubkey {
        self.position_replay.derivation.treasury_replay_account
    }

    /// Counted 0xbb service-ledger account.
    pub(crate) const fn treasury_service_ledger_account(self) -> Pubkey {
        self.service_ledger.account
    }
}

/// Hostile-authenticate the actual zero-liability ordinary PositionV3 and
/// mandatory initial GEN1 ReplayV3 written by the Product/General founder.
#[inline(never)]
pub(crate) fn authenticate_treasury_position_replay_foundation_v1<F>(
    program_id: &Pubkey,
    derivation: RevenueMarketTreasuryDerivationV1,
    market_facts: &F,
    position_account: &AccountInfo,
    replay_account: &AccountInfo,
) -> Outcome<AuthenticatedTreasuryPositionReplayFoundationV1>
where
    F: AuthenticatedTreasuryMarketFactsV1,
{
    require(
        market_facts.market_instance_v2_id() == Some(derivation.market_instance_v2_id)
            && market_facts.realm() == Some(derivation.authority.realm())
            && market_facts.general_market_runtime_account()
                == Some(derivation.general_market_runtime_account)
            && market_facts.collateral_policy_id().is_some()
            && market_facts.collateral_release_id().is_some()
            && market_facts.outcome_count().is_some(),
        ClutchError::MismatchedState,
    )?;
    require(
        *position_account.key == derivation.treasury_position_account
            && *replay_account.key == derivation.treasury_replay_account,
        ClutchError::WrongPda,
    )?;
    accounts::validate_state_role_lengths(
        program_id,
        position_account,
        true,
        &[POSITION_V3_BYTES],
    )?;
    accounts::validate_state_role_lengths(
        program_id,
        replay_account,
        true,
        &[GENERAL_REPLAY_ACCOUNT_V1_BYTES],
    )?;
    let position_data = position_account.data.borrow();
    let position = PositionAccountV3::decode(&position_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let replay_data = replay_account.data.borrow();
    let replay = ReplayV3Envelope::decode(&replay_data, &RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let position_rent = position.rent();
    let replay_rent = replay.header().rent();
    let plan = found_general_position_replay_v1(
        Identity32V1::new(position_account.key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(replay_account.key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(derivation.market_instance_v2_id.bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(derivation.authority.realm().bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(
            market_facts
                .collateral_policy_id()
                .ok_or(ClutchError::MismatchedState)?
                .bytes(),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(
            market_facts
                .collateral_release_id()
                .ok_or(ClutchError::MismatchedState)?
                .bytes(),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(derivation.authority.treasury_owner().bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(derivation.general_market_runtime_account.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        market_facts
            .outcome_count()
            .ok_or(ClutchError::MismatchedState)?,
        derivation.treasury_position_bump,
        derivation.treasury_replay_bump,
        position_rent,
        replay_rent,
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        plan.position_body().as_slice() == position_data.as_ref()
            && plan.replay_body().as_slice() == replay_data.as_ref(),
        ClutchError::MismatchedState,
    )?;
    let position_accounted = position_rent
        .refundable_live_principal
        .checked_add(position_rent.permanent_tombstone_principal)
        .and_then(|value| value.checked_add(position_rent.donation_floor))
        .ok_or(ClutchError::Arithmetic)?;
    let replay_accounted = replay_rent
        .refundable_principal()
        .checked_add(replay_rent.donation_floor())
        .ok_or(ClutchError::Arithmetic)?;
    require(
        position_account.lamports() >= position_accounted
            && replay_account.lamports() >= replay_accounted,
        ClutchError::AggregateClosureMismatch,
    )?;
    Ok(AuthenticatedTreasuryPositionReplayFoundationV1 {
        derivation,
        position_generation: GENERAL_POSITION_FOUNDING_GENERATION_V1,
        position_semantic_id: Hash32::from_bytes(plan.position_semantic_id().bytes()),
        replay_semantic_id: Hash32::from_bytes(plan.replay_semantic_id().bytes()),
    })
}

/// Create and independently rent-fund the canonical ordinary treasury
/// PositionV3, mandatory GEN1 ReplayV3, and 0xbb service ledger in one rollback
/// domain. The frozen Product ScheduleV3 does not fund any of these accounts;
/// `rent_payer` is an explicit signer and becomes the exact principal owner in
/// all three bodies. Hostile prefunds remain donation floors.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn found_revenue_market_treasury_v1<F>(
    program_id: &Pubkey,
    rent_payer: &AccountInfo,
    position_account: &AccountInfo,
    replay_account: &AccountInfo,
    service_ledger_account: &AccountInfo,
    system_program: &AccountInfo,
    rent: &genesis::RentParameters,
    derivation: RevenueMarketTreasuryDerivationV1,
    market_facts: &F,
) -> Outcome<RevenueMarketTreasuryFoundationV1>
where
    F: AuthenticatedTreasuryMarketFactsV1,
{
    require_signer(rent_payer)?;
    genesis::require_system_program(system_program)?;
    require(
        rent_payer.key != position_account.key
            && rent_payer.key != replay_account.key
            && rent_payer.key != service_ledger_account.key
            && rent_payer.key != system_program.key
            && position_account.key != replay_account.key
            && position_account.key != service_ledger_account.key
            && position_account.key != system_program.key
            && replay_account.key != service_ledger_account.key
            && replay_account.key != system_program.key
            && service_ledger_account.key != system_program.key,
        ClutchError::AccountAlias,
    )?;
    require(
        *position_account.key == derivation.treasury_position_account
            && *replay_account.key == derivation.treasury_replay_account
            && *service_ledger_account.key == derivation.treasury_service_ledger_account,
        ClutchError::WrongPda,
    )?;
    genesis::require_creatable(position_account)?;
    genesis::require_creatable(replay_account)?;
    genesis::require_creatable(service_ledger_account)?;

    let position_live_principal = rent.minimum_balance(POSITION_V3_BYTES)?;
    let position_tombstone_principal = rent.minimum_balance(POSITION_TOMBSTONE_V3_BYTES)?;
    let position_refundable_principal = position_live_principal
        .checked_sub(position_tombstone_principal)
        .ok_or(ClutchError::Arithmetic)?;
    let replay_principal = rent.minimum_balance(GENERAL_REPLAY_ACCOUNT_V1_BYTES)?;
    require(
        position_refundable_principal != 0
            && position_tombstone_principal != 0
            && replay_principal != 0,
        ClutchError::WrongRentSysvar,
    )?;
    let position_funding = direct_creation_funding(
        rent_payer,
        position_account,
        rent,
        POSITION_V3_BYTES,
        DIRECT_NEUTRAL_SINK_V3,
    )?;
    let replay_funding = direct_creation_funding(
        rent_payer,
        replay_account,
        rent,
        GENERAL_REPLAY_ACCOUNT_V1_BYTES,
        DIRECT_NEUTRAL_SINK_V3,
    )?;
    let payer_identity = Identity32V1::new(rent_payer.key.to_bytes())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let position_rent = RentSplitV2 {
        payer: payer_identity,
        refundable_live_principal: position_refundable_principal,
        permanent_tombstone_principal: position_tombstone_principal,
        donation_floor: position_funding.prior_donation_lamports,
    };
    position_rent
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let replay_rent = DeletableRentOwnerV1::from_persisted(
        payer_identity,
        replay_principal,
        replay_funding.prior_donation_lamports,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let collateral_policy = market_facts
        .collateral_policy_id()
        .ok_or(ClutchError::MismatchedState)?;
    let collateral_release = market_facts
        .collateral_release_id()
        .ok_or(ClutchError::MismatchedState)?;
    let outcome_count = market_facts
        .outcome_count()
        .ok_or(ClutchError::MismatchedState)?;
    require(
        market_facts.market_instance_v2_id() == Some(derivation.market_instance_v2_id)
            && market_facts.realm() == Some(derivation.authority.realm())
            && market_facts.general_market_runtime_account()
                == Some(derivation.general_market_runtime_account),
        ClutchError::MismatchedState,
    )?;
    let plan = found_general_position_replay_v1(
        Identity32V1::new(position_account.key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(replay_account.key.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(derivation.market_instance_v2_id.bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(derivation.authority.realm().bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(collateral_policy.bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(collateral_release.bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(derivation.authority.treasury_owner().bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(derivation.general_market_runtime_account.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        outcome_count,
        derivation.treasury_position_bump,
        derivation.treasury_replay_bump,
        position_rent,
        replay_rent,
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market_bytes = derivation.market_instance_v2_id.bytes();
    let owner_bytes = derivation.authority.treasury_owner().bytes();
    let runtime_bytes = derivation.general_market_runtime_account.to_bytes();
    let purpose_byte = [u8::from(PositionPurposeV3::General)];
    let position_bump = [derivation.treasury_position_bump];
    create_pda_account_full_principal(
        program_id,
        rent_payer,
        position_account,
        system_program,
        rent,
        POSITION_V3_BYTES,
        position_funding,
        0,
        &[
            clutch_retirement::POSITION_V3_PDA_PREFIX,
            &market_bytes,
            &owner_bytes,
            &purpose_byte,
            &runtime_bytes,
            &position_bump,
        ],
    )?;
    let position_bytes = position_account.key.to_bytes();
    let replay_bump = [derivation.treasury_replay_bump];
    create_pda_account_full_principal(
        program_id,
        rent_payer,
        replay_account,
        system_program,
        rent,
        GENERAL_REPLAY_ACCOUNT_V1_BYTES,
        replay_funding,
        0,
        &[
            clutch_retirement::PURPOSE_REPLAY_V3_PDA_PREFIX,
            &position_bytes,
            &purpose_byte,
            &runtime_bytes,
            &replay_bump,
        ],
    )?;
    position_account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(plan.position_body());
    replay_account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(plan.replay_body());
    let position_replay = authenticate_treasury_position_replay_foundation_v1(
        program_id,
        derivation,
        market_facts,
        position_account,
        replay_account,
    )?;
    let service_ledger = found_treasury_service_ledger_v1(
        program_id,
        rent_payer,
        service_ledger_account,
        system_program,
        rent,
        position_replay,
    )?;
    Ok(RevenueMarketTreasuryFoundationV1 {
        position_replay,
        service_ledger,
    })
}

fn product_funded_slot_data_id(
    domain: &[u8],
    account: &AccountInfo<'_>,
) -> Outcome<ContentId> {
    let data = account.try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[domain, account.key.as_ref(), &data]).to_bytes(),
    );
    require(id != ContentId::ZERO, ClutchError::MismatchedState)?;
    Ok(id)
}

fn treasury_position_plan_from_rent<F>(
    derivation: RevenueMarketTreasuryDerivationV1,
    market_facts: &F,
    position_rent: RentSplitV2,
) -> Outcome<clutch_general_v2_contract::GeneralPositionFoundingPlanV1>
where
    F: AuthenticatedTreasuryMarketFactsV1,
{
    let collateral_policy = market_facts.collateral_policy_id()
        .ok_or(ClutchError::MismatchedState)?;
    let collateral_release = market_facts.collateral_release_id()
        .ok_or(ClutchError::MismatchedState)?;
    let outcome_count = market_facts.outcome_count().ok_or(ClutchError::MismatchedState)?;
    require(
        market_facts.market_instance_v2_id() == Some(derivation.market_instance_v2_id)
            && market_facts.realm() == Some(derivation.authority.realm())
            && market_facts.general_market_runtime_account()
                == Some(derivation.general_market_runtime_account),
        ClutchError::MismatchedState,
    )?;
    found_general_position_v1(
        Identity32V1::new(derivation.treasury_position_account.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(derivation.treasury_replay_account.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(derivation.market_instance_v2_id.bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(derivation.authority.realm().bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(collateral_policy.bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(collateral_release.bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(derivation.authority.treasury_owner().bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(derivation.general_market_runtime_account.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        outcome_count, derivation.treasury_position_bump, position_rent, &RuntimeSha256,
    ).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

/// Write and hostile-reopen Product-funded ScheduleV4 slot 47.
#[inline(never)]
pub(crate) fn write_product_funded_treasury_position_v1<F>(
    program_id: &Pubkey,
    debit: &AuthenticatedProductMarketFoundationDebitV4,
    position_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent: &genesis::RentParameters,
    derivation: RevenueMarketTreasuryDerivationV1,
    market_facts: &F,
) -> Outcome<AuthenticatedProductFundedTreasurySlotV1>
where
    F: AuthenticatedTreasuryMarketFactsV1,
{
    require(
        debit.slot() == MarketFoundationSlotV4::GeneralTreasuryPosition
            && debit.destination_account() == derivation.treasury_position_account
            && *position_account.key == derivation.treasury_position_account,
        ClutchError::MismatchedState,
    )?;
    let live_principal = rent.minimum_balance(POSITION_V3_BYTES)?;
    let tombstone_principal = rent.minimum_balance(POSITION_TOMBSTONE_V3_BYTES)?;
    let refundable = live_principal.checked_sub(tombstone_principal)
        .ok_or(ClutchError::Arithmetic)?;
    require(debit.principal_lamports() == live_principal && refundable != 0,
        ClutchError::WrongRentSysvar)?;
    let position_rent = RentSplitV2 {
        payer: Identity32V1::new(debit.rent_refund_owner().to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        refundable_live_principal: refundable,
        permanent_tombstone_principal: tombstone_principal,
        donation_floor: debit.destination_donation_floor_lamports(),
    };
    position_rent.validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let plan = treasury_position_plan_from_rent(derivation, market_facts, position_rent)?;
    let market = derivation.market_instance_v2_id.bytes();
    let owner = derivation.authority.treasury_owner().bytes();
    let runtime = derivation.general_market_runtime_account.to_bytes();
    let purpose = [u8::from(PositionPurposeV3::General)];
    let bump = [derivation.treasury_position_bump];
    allocate_assign_product_funded_pda(
        program_id, position_account, system_program, POSITION_V3_BYTES,
        &[clutch_retirement::POSITION_V3_PDA_PREFIX, &market, &owner, &purpose, &runtime, &bump],
    )?;
    position_account.try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(plan.position_body());
    let decoded = PositionAccountV3::decode(&position_account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(decoded == plan.position(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedProductFundedTreasurySlotV1 {
        slot: MarketFoundationSlotV4::GeneralTreasuryPosition,
        account: *position_account.key,
        account_data_id: product_funded_slot_data_id(
            b"dragons-clutch/sbf/general-treasury-position/data/v3\0", position_account)?,
        semantic_id: Hash32::from_bytes(plan.position_semantic_id().bytes()),
    })
}

/// Write and hostile-reopen Product-funded ScheduleV4 slot 48 while
/// reauthenticating the exact slot-47 Position postimage.
#[inline(never)]
pub(crate) fn write_product_funded_treasury_replay_v1<F>(
    program_id: &Pubkey,
    debit: &AuthenticatedProductMarketFoundationDebitV4,
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent: &genesis::RentParameters,
    derivation: RevenueMarketTreasuryDerivationV1,
    market_facts: &F,
) -> Outcome<AuthenticatedProductFundedTreasurySlotV1>
where
    F: AuthenticatedTreasuryMarketFactsV1,
{
    require(
        debit.slot() == MarketFoundationSlotV4::GeneralTreasuryReplay
            && debit.destination_account() == derivation.treasury_replay_account
            && *replay_account.key == derivation.treasury_replay_account,
        ClutchError::MismatchedState,
    )?;
    accounts::validate_state_role_lengths(program_id, position_account, true, &[POSITION_V3_BYTES])?;
    let position = PositionAccountV3::decode(&position_account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let position_plan = treasury_position_plan_from_rent(derivation, market_facts, position.rent())?;
    require(position == position_plan.position(), ClutchError::MismatchedState)?;
    let replay_principal = rent.minimum_balance(GENERAL_REPLAY_ACCOUNT_V1_BYTES)?;
    require(debit.principal_lamports() == replay_principal, ClutchError::WrongRentSysvar)?;
    let replay_rent = DeletableRentOwnerV1::from_persisted(
        Identity32V1::new(debit.rent_refund_owner().to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        replay_principal, debit.destination_donation_floor_lamports(),
    ).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let replay_plan = found_general_replay_v1(
        Identity32V1::new(derivation.treasury_position_account.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(derivation.treasury_replay_account.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(derivation.authority.treasury_owner().bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        Identity32V1::new(derivation.general_market_runtime_account.to_bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        derivation.treasury_replay_bump, replay_rent,
        position_plan.position_semantic_id(), &RuntimeSha256,
    ).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let position_key = derivation.treasury_position_account.to_bytes();
    let runtime = derivation.general_market_runtime_account.to_bytes();
    let purpose = [u8::from(PositionPurposeV3::General)];
    let bump = [derivation.treasury_replay_bump];
    allocate_assign_product_funded_pda(
        program_id, replay_account, system_program, GENERAL_REPLAY_ACCOUNT_V1_BYTES,
        &[clutch_retirement::PURPOSE_REPLAY_V3_PDA_PREFIX, &position_key, &purpose, &runtime, &bump],
    )?;
    replay_account.try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(replay_plan.replay_body());
    let foundation = authenticate_treasury_position_replay_foundation_v1(
        program_id, derivation, market_facts, position_account, replay_account,
    )?;
    require(foundation.replay_semantic_id() == Hash32::from_bytes(
        replay_plan.replay_semantic_id().bytes()), ClutchError::MismatchedState)?;
    Ok(AuthenticatedProductFundedTreasurySlotV1 {
        slot: MarketFoundationSlotV4::GeneralTreasuryReplay,
        account: *replay_account.key,
        account_data_id: product_funded_slot_data_id(
            b"dragons-clutch/sbf/general-treasury-replay/data/v3\0", replay_account)?,
        semantic_id: foundation.replay_semantic_id(),
    })
}

/// Nonforgeable authentication of one live writable 0xbb ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedTreasuryServiceLedgerV1 {
    account: Pubkey,
    body: TreasuryServiceLedgerV1,
}

impl AuthenticatedTreasuryServiceLedgerV1 {
    /// Exact physical ledger account.
    pub(crate) const fn account(self) -> Pubkey {
        self.account
    }

    /// Exact hostile-decoded body.
    pub(crate) const fn body(self) -> TreasuryServiceLedgerV1 {
        self.body
    }
}

fn treasury_service_ledger_foundation_body_v1(
    foundation: AuthenticatedTreasuryPositionReplayFoundationV1,
    rent_payer: Hash32,
    refundable_rent_principal: u64,
    donation_floor: u64,
) -> Outcome<TreasuryServiceLedgerV1> {
    let authority = foundation.derivation.authority;
    let body = TreasuryServiceLedgerV1 {
        realm: authority.realm(),
        revenue_policy_record_account: Hash32::from_bytes(authority.record_account.to_bytes()),
        revenue_policy_record_v2_id: authority.record_semantic_id,
        market_instance_v2_id: foundation.derivation.market_instance_v2_id,
        treasury_owner: authority.treasury_owner(),
        treasury_position_account: Hash32::from_bytes(
            foundation.derivation.treasury_position_account.to_bytes(),
        ),
        treasury_position_founding_generation: foundation.position_generation,
        admitted_epoch_count: 0,
        settled_epoch_count: 0,
        rent_payer,
        refundable_rent_principal,
        donation_floor,
        stored_bump: foundation.derivation.treasury_service_ledger_bump,
        flags: 0,
    };
    body.validate()?;
    Ok(body)
}

/// Create and separately rent-fund the 0xbb ledger only after both ordinary
/// Position and mandatory Replay postimages have been authenticated.
#[inline(never)]
pub(crate) fn found_treasury_service_ledger_v1(
    program_id: &Pubkey,
    payer: &AccountInfo,
    ledger_account: &AccountInfo,
    system_program: &AccountInfo,
    rent: &genesis::RentParameters,
    foundation: AuthenticatedTreasuryPositionReplayFoundationV1,
) -> Outcome<AuthenticatedTreasuryServiceLedgerV1> {
    require_signer(payer)?;
    genesis::require_system_program(system_program)?;
    require(
        *ledger_account.key
            == foundation
                .derivation
                .treasury_service_ledger_account,
        ClutchError::WrongPda,
    )?;
    genesis::require_creatable(ledger_account)?;
    let funding = direct_creation_funding(
        payer,
        ledger_account,
        rent,
        TREASURY_SERVICE_LEDGER_V1_BYTES,
        DIRECT_NEUTRAL_SINK_V3,
    )?;
    let body = treasury_service_ledger_foundation_body_v1(
        foundation, funding.payer, funding.payer_principal_lamports,
        funding.prior_donation_lamports,
    )?;
    let market_bytes = foundation.derivation.market_instance_v2_id.bytes();
    let position_bytes = foundation.derivation.treasury_position_account.to_bytes();
    create_pda_account_full_principal(
        program_id,
        payer,
        ledger_account,
        system_program,
        rent,
        TREASURY_SERVICE_LEDGER_V1_BYTES,
        funding,
        0,
        &[
            seeds::SEED_TREASURY_SERVICE_LEDGER_V1,
            &market_bytes,
            &position_bytes,
            &[foundation.derivation.treasury_service_ledger_bump],
        ],
    )?;
    {
        let mut data = ledger_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        body.encode(&mut data)?;
        require(
            TreasuryServiceLedgerV1::decode(&data)? == body,
            ClutchError::MismatchedState,
        )?;
    }
    Ok(AuthenticatedTreasuryServiceLedgerV1 {
        account: *ledger_account.key,
        body,
    })
}

/// Write and hostile-reopen Product-funded ScheduleV4 slot 49 only after the
/// exact slot-47/48 Position and Replay pair has been reauthenticated.
#[inline(never)]
pub(crate) fn write_product_funded_treasury_service_ledger_v1<F>(
    program_id: &Pubkey,
    debit: &AuthenticatedProductMarketFoundationDebitV4,
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    ledger_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent: &genesis::RentParameters,
    derivation: RevenueMarketTreasuryDerivationV1,
    market_facts: &F,
) -> Outcome<AuthenticatedProductFundedTreasurySlotV1>
where
    F: AuthenticatedTreasuryMarketFactsV1,
{
    require(
        debit.slot() == MarketFoundationSlotV4::TreasuryServiceLedger
            && debit.destination_account() == derivation.treasury_service_ledger_account
            && *ledger_account.key == derivation.treasury_service_ledger_account,
        ClutchError::MismatchedState,
    )?;
    let foundation = authenticate_treasury_position_replay_foundation_v1(
        program_id, derivation, market_facts, position_account, replay_account,
    )?;
    let principal = rent.minimum_balance(TREASURY_SERVICE_LEDGER_V1_BYTES)?;
    require(debit.principal_lamports() == principal, ClutchError::WrongRentSysvar)?;
    let body = treasury_service_ledger_foundation_body_v1(
        foundation,
        Hash32::from_bytes(debit.rent_refund_owner().to_bytes()),
        principal,
        debit.destination_donation_floor_lamports(),
    )?;
    let market = derivation.market_instance_v2_id.bytes();
    let position = derivation.treasury_position_account.to_bytes();
    let bump = [derivation.treasury_service_ledger_bump];
    allocate_assign_product_funded_pda(
        program_id, ledger_account, system_program, TREASURY_SERVICE_LEDGER_V1_BYTES,
        &[seeds::SEED_TREASURY_SERVICE_LEDGER_V1, &market, &position, &bump],
    )?;
    {
        let mut data = ledger_account.try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        body.encode(&mut data)?;
        require(TreasuryServiceLedgerV1::decode(&data)? == body,
            ClutchError::MismatchedState)?;
    }
    let data_id = product_funded_slot_data_id(
        b"dragons-clutch/sbf/treasury-service-ledger/data/v1\0", ledger_account)?;
    let data = ledger_account.try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let semantic_id = Hash32::from_bytes(solana_sha256_hasher::hashv(&[
        b"dragons-clutch/treasury-service-ledger/body/v1\0", &data,
    ]).to_bytes());
    drop(data);
    require(semantic_id != Hash32::ZERO, ClutchError::MismatchedState)?;
    Ok(AuthenticatedProductFundedTreasurySlotV1 {
        slot: MarketFoundationSlotV4::TreasuryServiceLedger,
        account: *ledger_account.key,
        account_data_id: data_id,
        semantic_id,
    })
}

/// Authenticate a live 0xbb ledger against the exact immutable Realm/Market
/// derivation. The initial Position/Replay postimage is deliberately not
/// required here: real fee collection advances both mutable bodies after
/// founding. `writable` is explicit so read-only observation can never be
/// promoted into mutation authority.
#[inline(never)]
pub(crate) fn authenticate_treasury_service_ledger_v1(
    program_id: &Pubkey,
    ledger_account: &AccountInfo,
    derivation: RevenueMarketTreasuryDerivationV1,
    writable: bool,
) -> Outcome<AuthenticatedTreasuryServiceLedgerV1> {
    accounts::validate_state_role_lengths(
        program_id,
        ledger_account,
        writable,
        &[TREASURY_SERVICE_LEDGER_V1_BYTES],
    )?;
    require(
        *ledger_account.key == derivation.treasury_service_ledger_account,
        ClutchError::WrongPda,
    )?;
    let body = TreasuryServiceLedgerV1::decode(&ledger_account.data.borrow())?;
    let authority = derivation.authority;
    require(
        body.realm == authority.realm()
            && body.revenue_policy_record_account
                == Hash32::from_bytes(authority.record_account.to_bytes())
            && body.revenue_policy_record_v2_id == authority.record_semantic_id
            && body.market_instance_v2_id == derivation.market_instance_v2_id
            && body.treasury_owner == authority.treasury_owner()
            && body.treasury_position_account
                == Hash32::from_bytes(derivation.treasury_position_account.to_bytes())
            && body.treasury_position_founding_generation
                == GENERAL_POSITION_FOUNDING_GENERATION_V1,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        ledger_account.key,
        seeds::treasury_service_ledger_v1_pda(
            program_id,
            &body.market_instance_v2_id.bytes(),
            &derivation.treasury_position_account,
        ),
        Some(body.stored_bump),
    )?;
    let accounted = body
        .refundable_rent_principal
        .checked_add(body.donation_floor)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        ledger_account.lamports() >= accounted,
        ClutchError::AggregateClosureMismatch,
    )?;
    Ok(AuthenticatedTreasuryServiceLedgerV1 {
        account: *ledger_account.key,
        body,
    })
}

/// Default-refusing General authority for exactly one fee-bearing Epoch
/// admission.  SettlementRoot/Epoch state owns the per-epoch replay bit; the
/// ledger owns only aggregate conservation.
pub(crate) trait AuthenticatedTreasuryServiceAdmissionV1 {
    /// Realm identity.
    fn realm(&self) -> Option<Hash32> { None }
    /// MarketInstanceV2 identity.
    fn market_instance_v2_id(&self) -> Option<Hash32> { None }
    /// Revenue record account.
    fn revenue_policy_record_account(&self) -> Option<Pubkey> { None }
    /// Revenue record semantic ID.
    fn revenue_policy_record_v2_id(&self) -> Option<Hash32> { None }
    /// RevenuePolicyV2 digest.
    fn revenue_policy_v2_digest(&self) -> Option<Hash32> { None }
    /// Treasury owner.
    fn treasury_owner(&self) -> Option<Hash32> { None }
    /// Treasury Position account.
    fn treasury_position_account(&self) -> Option<Pubkey> { None }
    /// Service-ledger account.
    fn treasury_service_ledger_account(&self) -> Option<Pubkey> { None }
    /// Exact General Epoch semantic identity whose root owns replay.
    fn epoch_semantic_id(&self) -> Option<Hash32> { None }
    /// Exact admitted count observed before the atomic General transition.
    fn admitted_epoch_count_before(&self) -> Option<u64> { None }
    /// Exact settled count observed before the atomic General transition.
    fn settled_epoch_count_before(&self) -> Option<u64> { None }
}

/// Default-refusing General authority for exactly one terminal fee-bearing
/// Epoch service.
pub(crate) trait AuthenticatedTreasuryServiceSettlementV1:
    AuthenticatedTreasuryServiceAdmissionV1
{
    /// Whether General proved this epoch's service terminal exactly once.
    fn service_is_terminal(&self) -> Option<bool> { None }
}

/// Kind of one exact counted ledger transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TreasuryServiceTransitionKindV1 {
    /// Increment admitted count.
    AdmitEpoch,
    /// Increment settled count.
    SettleEpoch,
}

/// Private compare-and-write plan for one exact 0xbb count transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreparedTreasuryServiceTransitionV1 {
    account: Pubkey,
    before: TreasuryServiceLedgerV1,
    after: TreasuryServiceLedgerV1,
    epoch_semantic_id: Hash32,
    transition_id: Hash32,
    kind: TreasuryServiceTransitionKindV1,
}

impl PreparedTreasuryServiceTransitionV1 {
    /// Exact postwrite body.
    pub(crate) const fn after(self) -> TreasuryServiceLedgerV1 { self.after }
    /// Exact transition receipt identity.
    pub(crate) const fn transition_id(self) -> Hash32 { self.transition_id }
    /// Exact epoch semantic identity bound by the transition.
    pub(crate) const fn epoch_semantic_id(self) -> Hash32 { self.epoch_semantic_id }
    /// Transition kind.
    pub(crate) const fn kind(self) -> TreasuryServiceTransitionKindV1 { self.kind }
}

fn require_service_evidence<E: AuthenticatedTreasuryServiceAdmissionV1>(
    authenticated: AuthenticatedTreasuryServiceLedgerV1,
    derivation: RevenueMarketTreasuryDerivationV1,
    evidence: &E,
) -> Outcome<Hash32> {
    let authority = derivation.authority;
    let epoch = evidence
        .epoch_semantic_id()
        .ok_or(ClutchError::MismatchedState)?;
    require(
        epoch != Hash32::ZERO
            && evidence.realm() == Some(authority.realm())
            && evidence.market_instance_v2_id()
                == Some(derivation.market_instance_v2_id)
            && evidence.revenue_policy_record_account() == Some(authority.record_account)
            && evidence.revenue_policy_record_v2_id() == Some(authority.record_semantic_id)
            && evidence.revenue_policy_v2_digest() == Some(authority.policy_digest)
            && evidence.treasury_owner() == Some(authority.treasury_owner())
            && evidence.treasury_position_account()
                == Some(derivation.treasury_position_account)
            && evidence.treasury_service_ledger_account() == Some(authenticated.account)
            && evidence.admitted_epoch_count_before()
                == Some(authenticated.body.admitted_epoch_count)
            && evidence.settled_epoch_count_before()
                == Some(authenticated.body.settled_epoch_count),
        ClutchError::MismatchedState,
    )?;
    Ok(epoch)
}

/// Prepare one exact aggregate admission from private General evidence.
pub(crate) fn prepare_treasury_service_admission_v1<E>(
    authenticated: AuthenticatedTreasuryServiceLedgerV1,
    derivation: RevenueMarketTreasuryDerivationV1,
    evidence: &E,
) -> Outcome<PreparedTreasuryServiceTransitionV1>
where
    E: AuthenticatedTreasuryServiceAdmissionV1,
{
    let epoch = require_service_evidence(authenticated, derivation, evidence)?;
    let after = authenticated.body.admit_epoch()?;
    prepare_service_transition(
        authenticated,
        after,
        epoch,
        TreasuryServiceTransitionKindV1::AdmitEpoch,
    )
}

/// Prepare one exact aggregate settlement from private terminal General
/// evidence.
pub(crate) fn prepare_treasury_service_settlement_v1<E>(
    authenticated: AuthenticatedTreasuryServiceLedgerV1,
    derivation: RevenueMarketTreasuryDerivationV1,
    evidence: &E,
) -> Outcome<PreparedTreasuryServiceTransitionV1>
where
    E: AuthenticatedTreasuryServiceSettlementV1,
{
    let epoch = require_service_evidence(authenticated, derivation, evidence)?;
    require(
        evidence.service_is_terminal() == Some(true),
        ClutchError::MismatchedState,
    )?;
    let after = authenticated.body.settle_epoch()?;
    prepare_service_transition(
        authenticated,
        after,
        epoch,
        TreasuryServiceTransitionKindV1::SettleEpoch,
    )
}

fn prepare_service_transition(
    authenticated: AuthenticatedTreasuryServiceLedgerV1,
    after: TreasuryServiceLedgerV1,
    epoch: Hash32,
    kind: TreasuryServiceTransitionKindV1,
) -> Outcome<PreparedTreasuryServiceTransitionV1> {
    let (domain, kind_byte) = match kind {
        TreasuryServiceTransitionKindV1::AdmitEpoch => {
            (TREASURY_SERVICE_ADMISSION_DOMAIN_V1, 1u8)
        }
        TreasuryServiceTransitionKindV1::SettleEpoch => {
            (TREASURY_SERVICE_SETTLEMENT_DOMAIN_V1, 2u8)
        }
    };
    let transition_id = Hash32::from_bytes(
        solana_sha256_hasher::hashv(&[
            domain,
            &authenticated.account.to_bytes(),
            &epoch.bytes(),
            &authenticated.body.admitted_epoch_count.to_le_bytes(),
            &authenticated.body.settled_epoch_count.to_le_bytes(),
            &after.admitted_epoch_count.to_le_bytes(),
            &after.settled_epoch_count.to_le_bytes(),
            &[kind_byte],
        ])
        .to_bytes(),
    );
    require(transition_id != Hash32::ZERO, ClutchError::MismatchedState)?;
    Ok(PreparedTreasuryServiceTransitionV1 {
        account: authenticated.account,
        before: authenticated.body,
        after,
        epoch_semantic_id: epoch,
        transition_id,
        kind,
    })
}

/// Compare the actual current body, write one prepared transition, and
/// hostile-decode the postimage.
pub(crate) fn accept_treasury_service_transition_v1(
    account: &AccountInfo,
    prepared: PreparedTreasuryServiceTransitionV1,
) -> Outcome<AuthenticatedTreasuryServiceLedgerV1> {
    require(
        *account.key == prepared.account && account.is_writable,
        ClutchError::MismatchedState,
    )?;
    {
        let data = account.data.borrow();
        require(
            TreasuryServiceLedgerV1::decode(&data)? == prepared.before,
            ClutchError::Replay,
        )?;
    }
    {
        let mut data = account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        prepared.after.encode(&mut data)?;
        require(
            TreasuryServiceLedgerV1::decode(&data)? == prepared.after,
            ClutchError::MismatchedState,
        )?;
    }
    Ok(AuthenticatedTreasuryServiceLedgerV1 {
        account: prepared.account,
        body: prepared.after,
    })
}

/// Default-refusing Market/Position terminal authority for 0xbb rent close.
pub(crate) trait AuthenticatedTreasuryServiceCloseV1 {
    /// Exact ledger account.
    fn treasury_service_ledger_account(&self) -> Option<Pubkey> { None }
    /// Full MarketInstanceV2 identity.
    fn market_instance_v2_id(&self) -> Option<Hash32> { None }
    /// Treasury Position account.
    fn treasury_position_account(&self) -> Option<Pubkey> { None }
    /// Exact terminal Market/Position receipt ID.
    fn terminal_receipt_id(&self) -> Option<Hash32> { None }
}

/// Private proof that the counted service ledger was deleted with the exact
/// payer/sink split.  Product/Position close consumes this ID; the physical
/// ledger need not remain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TreasuryServiceLedgerCloseReceiptV1 {
    close_receipt_id: Hash32,
    refunded_principal: u64,
    neutral_lamports: u64,
}

impl TreasuryServiceLedgerCloseReceiptV1 {
    /// Exact close receipt ID.
    pub(crate) const fn close_receipt_id(self) -> Hash32 { self.close_receipt_id }
    /// Principal returned to the persisted payer.
    pub(crate) const fn refunded_principal(self) -> u64 { self.refunded_principal }
    /// Donation and surplus sent to the neutral sink.
    pub(crate) const fn neutral_lamports(self) -> u64 { self.neutral_lamports }
}

/// Close an economically exhausted 0xbb ledger under private terminal
/// authority.  This must precede ordinary Position/Replay retirement.
pub(crate) fn close_treasury_service_ledger_v1<C>(
    ledger_account: &AccountInfo,
    authenticated: AuthenticatedTreasuryServiceLedgerV1,
    derivation: RevenueMarketTreasuryDerivationV1,
    terminal: &C,
    payer: &AccountInfo,
    sink: &AccountInfo,
) -> Outcome<TreasuryServiceLedgerCloseReceiptV1>
where
    C: AuthenticatedTreasuryServiceCloseV1,
{
    let terminal_receipt_id = terminal
        .terminal_receipt_id()
        .ok_or(ClutchError::MismatchedState)?;
    require(
        terminal_receipt_id != Hash32::ZERO
            && terminal.treasury_service_ledger_account() == Some(authenticated.account)
            && terminal.market_instance_v2_id() == Some(derivation.market_instance_v2_id)
            && terminal.treasury_position_account()
                == Some(derivation.treasury_position_account)
            && authenticated.body.is_economically_closeable()
            && *ledger_account.key == authenticated.account
            && ledger_account.is_writable
            && Hash32::from_bytes(payer.key.to_bytes()) == authenticated.body.rent_payer
            && payer.is_writable
            && *sink.key == incinerator::ID
            && sink.is_writable
            && ledger_account.key != payer.key
            && ledger_account.key != sink.key
            && payer.key != sink.key,
        ClutchError::MismatchedState,
    )?;
    let before = TreasuryServiceLedgerV1::decode(&ledger_account.data.borrow())?;
    require(before == authenticated.body, ClutchError::Replay)?;
    let observed = ledger_account.lamports();
    let neutral = observed
        .checked_sub(before.refundable_rent_principal)
        .ok_or(ClutchError::AggregateClosureMismatch)?;
    require(neutral >= before.donation_floor, ClutchError::AggregateClosureMismatch)?;
    let close_receipt_id = Hash32::from_bytes(
        solana_sha256_hasher::hashv(&[
            TREASURY_SERVICE_CLOSE_DOMAIN_V1,
            &authenticated.account.to_bytes(),
            &terminal_receipt_id.bytes(),
            &before.market_instance_v2_id.bytes(),
            &before.treasury_position_account.bytes(),
            &before.admitted_epoch_count.to_le_bytes(),
            &before.settled_epoch_count.to_le_bytes(),
            &before.refundable_rent_principal.to_le_bytes(),
            &neutral.to_le_bytes(),
        ])
        .to_bytes(),
    );
    {
        let mut lamports = ledger_account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **lamports = 0;
    }
    ledger_account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    ledger_account.assign(&genesis::SYSTEM_PROGRAM_ID);
    credit_lamports(payer, before.refundable_rent_principal)?;
    credit_lamports(sink, neutral)?;
    Ok(TreasuryServiceLedgerCloseReceiptV1 {
        close_receipt_id,
        refunded_principal: before.refundable_rent_principal,
        neutral_lamports: neutral,
    })
}

/// Authenticate exact current Realm/record accounts and an untrusted carrier
/// of the canonical policy bytes.  The carrier has no authority: digest and
/// copied-field equality to the immutable program-owned record do.
#[inline(never)]
pub(crate) fn authenticate_revenue_policy_record_v2(
    program_id: &Pubkey,
    realm_account: &AccountInfo,
    record_account: &AccountInfo,
    policy_preimage: &[u8],
) -> Outcome<AuthenticatedRevenuePolicyRecordV2> {
    accounts::validate_state_role_lengths(
        program_id,
        realm_account,
        false,
        &[account_len::REALM],
    )?;
    let realm = RealmAccount::decode(&realm_account.data.borrow())?;
    expect_pda(
        realm_account.key,
        seeds::realm_pda(program_id, &realm.realm.bytes()),
        Some(realm.stored_bump),
    )?;

    accounts::validate_state_role_lengths(
        program_id,
        record_account,
        false,
        &[REVENUE_POLICY_RECORD_BYTES_V2],
    )?;
    let record = RevenuePolicyRecordV2::decode(&record_account.data.borrow())?;
    require(record.realm == realm.realm, ClutchError::MismatchedState)?;
    expect_pda(
        record_account.key,
        seeds::revenue_policy_pda(program_id, &realm.realm.bytes()),
        Some(record.stored_bump),
    )?;

    let policy = decode_revenue_policy_v2(policy_preimage)
        .map_err(|_| Refusal::Adapter(ClutchError::EvidenceBufferMismatch))?;
    record.binds_policy(&policy)?;
    let digest = revenue_policy_v2_digest(&policy)
        .map_err(|_| Refusal::Adapter(ClutchError::EvidenceBufferMismatch))?;
    let semantic_id = revenue_policy_record_v2_id(realm.realm.bytes(), &policy)
        .map_err(|_| Refusal::Adapter(ClutchError::EvidenceBufferMismatch))?;
    let derivation_id =
        treasury_position_derivation_policy_v2_id(policy.treasury_position_derivation);
    Ok(AuthenticatedRevenuePolicyRecordV2 {
        realm: realm.realm,
        record_account: *record_account.key,
        record_semantic_id: Hash32::from_bytes(semantic_id.0),
        policy_digest: Hash32::from_bytes(digest.0),
        policy,
        treasury_position_derivation_policy_id: Hash32::from_bytes(derivation_id.0),
    })
}

/// Derive the exact two Market-scoped treasury accounts from an authenticated
/// Realm authority and full MarketInstanceV2 identity.
pub(crate) fn derive_revenue_market_treasury_v1(
    program_id: &Pubkey,
    authority: AuthenticatedRevenuePolicyRecordV2,
    market_instance_v2_id: Hash32,
    general_market_runtime_account: Pubkey,
) -> Outcome<RevenueMarketTreasuryDerivationV1> {
    require(
        market_instance_v2_id != Hash32::ZERO,
        ClutchError::MismatchedState,
    )?;
    require(
        general_market_runtime_account != Pubkey::new_from_array([0; 32]),
        ClutchError::MismatchedState,
    )?;
    let market_bytes = market_instance_v2_id.bytes();
    let treasury_owner = authority.treasury_owner().bytes();
    let runtime_bytes = general_market_runtime_account.to_bytes();
    let (treasury_position_account, treasury_position_bump) = seeds::position_v3_pda(
        program_id,
        &market_bytes,
        &treasury_owner,
        PositionPurposeV3::General,
        &runtime_bytes,
    );
    let (treasury_replay_account, treasury_replay_bump) = seeds::purpose_replay_v3_pda(
        program_id,
        &treasury_position_account.to_bytes(),
        PositionPurposeV3::General,
        &runtime_bytes,
    );
    let (treasury_service_ledger_account, treasury_service_ledger_bump) =
        seeds::treasury_service_ledger_v1_pda(
            program_id,
            &market_bytes,
            &treasury_position_account,
        );
    Ok(RevenueMarketTreasuryDerivationV1 {
        authority,
        market_instance_v2_id,
        general_market_runtime_account,
        treasury_position_account,
        treasury_position_bump,
        treasury_replay_account,
        treasury_replay_bump,
        treasury_service_ledger_account,
        treasury_service_ledger_bump,
    })
}

/// Execute one already-decoded Realm/revenue action.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    action: RealmRevenueV2Action,
    payload: &[u8],
) -> Outcome<()> {
    match action {
        RealmRevenueV2Action::InitializeFeeBearingRealmV2 => {
            initialize_fee_bearing_realm_v2(program_id, accounts, sequence, payload)
        }
        RealmRevenueV2Action::CloseRevenuePolicyRecordV2 => {
            close_revenue_policy_record_v2(program_id, accounts, sequence, payload)
        }
    }
}

#[inline(never)]
fn initialize_fee_bearing_realm_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require_count(accounts, INITIALIZE_FEE_BEARING_REALM_V2_ACCOUNT_COUNT)?;
    require_signer(&accounts[IX_REVENUE_V2_PAYER])?;
    require_distinct(accounts)?;
    require(sequence == 0, ClutchError::Replay)?;
    let request = InitializeFeeBearingRealmV2Payload::decode(payload)?;
    require(
        request.policy.is_successor_development_profile(),
        ClutchError::AuthorizationUnavailable,
    )?;
    genesis::require_system_program(&accounts[IX_REVENUE_V2_SYSTEM])?;
    let rent = genesis::read_rent(&accounts[IX_REVENUE_V2_RENT])?;
    let (profile, _, _) = genesis::read_canonical_policy(
        program_id,
        &accounts[IX_REVENUE_V2_COLLATERAL_POLICY],
    )?;
    require(profile == request.profile, ClutchError::EvidenceBufferMismatch)?;

    let realm = canonical_realm_id(request.profile, request.realm_nonce);
    let realm_bytes = realm.bytes();
    let (realm_address, realm_bump) = seeds::realm_pda(program_id, &realm_bytes);
    expect_pda(
        accounts[IX_REVENUE_V2_REALM].key,
        (realm_address, realm_bump),
        None,
    )?;
    let (record_address, record_bump) = seeds::revenue_policy_pda(program_id, &realm_bytes);
    expect_pda(
        accounts[IX_REVENUE_V2_RECORD].key,
        (record_address, record_bump),
        None,
    )?;
    genesis::require_creatable(&accounts[IX_REVENUE_V2_REALM])?;
    genesis::require_creatable(&accounts[IX_REVENUE_V2_RECORD])?;

    let record_funding = direct_creation_funding(
        &accounts[IX_REVENUE_V2_PAYER],
        &accounts[IX_REVENUE_V2_RECORD],
        &rent,
        REVENUE_POLICY_RECORD_BYTES_V2,
        DIRECT_NEUTRAL_SINK_V3,
    )?;
    let policy_digest = revenue_policy_v2_digest(&request.policy)
        .map_err(|_| Refusal::Adapter(ClutchError::EvidenceBufferMismatch))?;
    let record = RevenuePolicyRecordV2 {
        realm,
        policy_digest: Hash32::from_bytes(policy_digest.0),
        treasury_owner: Hash32::from_bytes(request.policy.treasury_owner),
        treasury_position_derivation: request.policy.treasury_position_derivation,
        terminal_payer: record_funding.payer,
        terminal_payer_principal: record_funding.payer_principal_lamports,
        terminal_donation_floor: record_funding.prior_donation_lamports,
        terminal_generation: 1,
        stored_bump: record_bump,
        flags: 0,
    };
    record.binds_policy(&request.policy)?;

    genesis::create_pda_account(
        program_id,
        &accounts[IX_REVENUE_V2_PAYER],
        &accounts[IX_REVENUE_V2_REALM],
        &accounts[IX_REVENUE_V2_SYSTEM],
        &rent,
        account_len::REALM,
        &[seeds::SEED_REALM, &realm_bytes, &[realm_bump]],
    )?;
    let realm_value = RealmAccount {
        realm,
        profile: request.profile,
        max_outcomes: request.max_outcomes,
        profile_version: request.profile_version,
        stored_bump: realm_bump,
        flags: 0,
    };
    {
        let mut data = accounts[IX_REVENUE_V2_REALM]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        realm_value.encode(&mut data)?;
        require(
            RealmAccount::decode(&data)? == realm_value,
            ClutchError::MismatchedState,
        )?;
    }

    create_pda_account_full_principal(
        program_id,
        &accounts[IX_REVENUE_V2_PAYER],
        &accounts[IX_REVENUE_V2_RECORD],
        &accounts[IX_REVENUE_V2_SYSTEM],
        &rent,
        REVENUE_POLICY_RECORD_BYTES_V2,
        record_funding,
        0,
        &[seeds::SEED_REVENUE_POLICY, &realm_bytes, &[record_bump]],
    )?;
    {
        let mut data = accounts[IX_REVENUE_V2_RECORD]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        record.encode(&mut data)?;
        let written = RevenuePolicyRecordV2::decode(&data)?;
        written.binds_policy(&request.policy)?;
        require(written == record, ClutchError::MismatchedState)?;
    }
    Ok(())
}

#[inline(never)]
fn close_revenue_policy_record_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require_count(accounts, CLOSE_REVENUE_POLICY_RECORD_V2_ACCOUNT_COUNT)?;
    require_distinct(accounts)?;
    require(sequence == 0, ClutchError::Replay)?;
    let request = CloseRevenuePolicyRecordV2Payload::decode(payload)?;
    let realm_bytes = request.realm.bytes();
    let (realm_address, _) = seeds::realm_pda(program_id, &realm_bytes);
    require(
        *accounts[IX_CLOSE_REVENUE_V2_REALM].key == realm_address,
        ClutchError::WrongPda,
    )?;
    require(
        accounts[IX_CLOSE_REVENUE_V2_REALM].data_len() == 0
            && *accounts[IX_CLOSE_REVENUE_V2_REALM].owner == genesis::SYSTEM_PROGRAM_ID,
        ClutchError::MismatchedState,
    )?;

    accounts::validate_state_role_lengths(
        program_id,
        &accounts[IX_CLOSE_REVENUE_V2_RECORD],
        true,
        &[REVENUE_POLICY_RECORD_BYTES_V2],
    )?;
    let record = RevenuePolicyRecordV2::decode(
        &accounts[IX_CLOSE_REVENUE_V2_RECORD].data.borrow(),
    )?;
    require(record.realm == request.realm, ClutchError::MismatchedState)?;
    expect_pda(
        accounts[IX_CLOSE_REVENUE_V2_RECORD].key,
        seeds::revenue_policy_pda(program_id, &realm_bytes),
        Some(record.stored_bump),
    )?;
    require(
        Hash32::from_bytes(accounts[IX_CLOSE_REVENUE_V2_PAYER].key.to_bytes())
            == record.terminal_payer,
        ClutchError::MismatchedState,
    )?;
    require(
        accounts[IX_CLOSE_REVENUE_V2_PAYER].is_writable,
        ClutchError::NotWritable,
    )?;
    require(
        *accounts[IX_CLOSE_REVENUE_V2_SINK].key == incinerator::ID,
        ClutchError::MismatchedState,
    )?;
    require(
        accounts[IX_CLOSE_REVENUE_V2_SINK].is_writable,
        ClutchError::NotWritable,
    )?;

    let observed = accounts[IX_CLOSE_REVENUE_V2_RECORD].lamports();
    let neutral = observed
        .checked_sub(record.terminal_payer_principal)
        .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?;
    require(
        neutral >= record.terminal_donation_floor,
        ClutchError::AggregateClosureMismatch,
    )?;
    {
        let mut lamports = accounts[IX_CLOSE_REVENUE_V2_RECORD]
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **lamports = 0;
    }
    accounts[IX_CLOSE_REVENUE_V2_RECORD]
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    accounts[IX_CLOSE_REVENUE_V2_RECORD].assign(&genesis::SYSTEM_PROGRAM_ID);
    credit_lamports(
        &accounts[IX_CLOSE_REVENUE_V2_PAYER],
        record.terminal_payer_principal,
    )?;
    credit_lamports(&accounts[IX_CLOSE_REVENUE_V2_SINK], neutral)
}

fn credit_lamports(account: &AccountInfo, amount: u64) -> Outcome<()> {
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = lamports
        .checked_add(amount)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_batch_policy_identity::revenue_policy_v2::{
        canonical_revenue_policy_v2_bytes, SUCCESSOR_DEV_DISPERSION_BPS,
        SUCCESSOR_DEV_FLOOR_RANGE_BPS,
    };
    use clutch_solana_layout::revenue::{
        CLOSE_REVENUE_POLICY_RECORD_V2_PAYLOAD_BYTES,
        INITIALIZE_FEE_BEARING_REALM_V2_PAYLOAD_BYTES,
    };

    #[test]
    fn payload_and_account_contracts_are_exact() {
        assert_eq!(INITIALIZE_FEE_BEARING_REALM_V2_PAYLOAD_BYTES, 122);
        assert_eq!(CLOSE_REVENUE_POLICY_RECORD_V2_PAYLOAD_BYTES, 32);
        assert_eq!(INITIALIZE_FEE_BEARING_REALM_V2_ACCOUNT_COUNT, 6);
        assert_eq!(CLOSE_REVENUE_POLICY_RECORD_V2_ACCOUNT_COUNT, 4);
        let policy = RevenuePolicyV2::successor_development([7; 32]);
        assert_eq!(policy.dispersion_bps, SUCCESSOR_DEV_DISPERSION_BPS);
        assert_eq!(policy.floor_range_bps, SUCCESSOR_DEV_FLOOR_RANGE_BPS);
        assert_eq!(canonical_revenue_policy_v2_bytes(&policy).unwrap().len(), 80);
    }

    #[derive(Clone, Copy)]
    struct TestServiceEvidence {
        realm: Hash32,
        market: Hash32,
        record_account: Pubkey,
        record_id: Hash32,
        policy: Hash32,
        owner: Hash32,
        position: Pubkey,
        ledger: Pubkey,
        epoch: Hash32,
        admitted_before: u64,
        settled_before: u64,
        terminal: bool,
    }

    impl AuthenticatedTreasuryServiceAdmissionV1 for TestServiceEvidence {
        fn realm(&self) -> Option<Hash32> { Some(self.realm) }
        fn market_instance_v2_id(&self) -> Option<Hash32> { Some(self.market) }
        fn revenue_policy_record_account(&self) -> Option<Pubkey> { Some(self.record_account) }
        fn revenue_policy_record_v2_id(&self) -> Option<Hash32> { Some(self.record_id) }
        fn revenue_policy_v2_digest(&self) -> Option<Hash32> { Some(self.policy) }
        fn treasury_owner(&self) -> Option<Hash32> { Some(self.owner) }
        fn treasury_position_account(&self) -> Option<Pubkey> { Some(self.position) }
        fn treasury_service_ledger_account(&self) -> Option<Pubkey> { Some(self.ledger) }
        fn epoch_semantic_id(&self) -> Option<Hash32> { Some(self.epoch) }
        fn admitted_epoch_count_before(&self) -> Option<u64> { Some(self.admitted_before) }
        fn settled_epoch_count_before(&self) -> Option<u64> { Some(self.settled_before) }
    }

    impl AuthenticatedTreasuryServiceSettlementV1 for TestServiceEvidence {
        fn service_is_terminal(&self) -> Option<bool> { Some(self.terminal) }
    }

    fn service_transition_fixture() -> (
        AuthenticatedTreasuryServiceLedgerV1,
        RevenueMarketTreasuryDerivationV1,
        TestServiceEvidence,
    ) {
        let realm = Hash32::from_bytes([1; 32]);
        let market = Hash32::from_bytes([2; 32]);
        let owner = Hash32::from_bytes([3; 32]);
        let record_account = Pubkey::new_from_array([4; 32]);
        let position = Pubkey::new_from_array([5; 32]);
        let replay = Pubkey::new_from_array([6; 32]);
        let runtime = Pubkey::new_from_array([7; 32]);
        let ledger = Pubkey::new_from_array([8; 32]);
        let record_id = Hash32::from_bytes([9; 32]);
        let policy = Hash32::from_bytes([10; 32]);
        let authority = AuthenticatedRevenuePolicyRecordV2 {
            realm,
            record_account,
            record_semantic_id: record_id,
            policy_digest: policy,
            policy: RevenuePolicyV2::successor_development(owner.bytes()),
            treasury_position_derivation_policy_id: Hash32::from_bytes([11; 32]),
        };
        let derivation = RevenueMarketTreasuryDerivationV1 {
            authority,
            market_instance_v2_id: market,
            general_market_runtime_account: runtime,
            treasury_position_account: position,
            treasury_position_bump: 250,
            treasury_replay_account: replay,
            treasury_replay_bump: 249,
            treasury_service_ledger_account: ledger,
            treasury_service_ledger_bump: 248,
        };
        let body = TreasuryServiceLedgerV1 {
            realm,
            revenue_policy_record_account: Hash32::from_bytes(record_account.to_bytes()),
            revenue_policy_record_v2_id: record_id,
            market_instance_v2_id: market,
            treasury_owner: owner,
            treasury_position_account: Hash32::from_bytes(position.to_bytes()),
            treasury_position_founding_generation: GENERAL_POSITION_FOUNDING_GENERATION_V1,
            admitted_epoch_count: 0,
            settled_epoch_count: 0,
            rent_payer: Hash32::from_bytes([12; 32]),
            refundable_rent_principal: 1,
            donation_floor: 0,
            stored_bump: 248,
            flags: 0,
        };
        let authenticated = AuthenticatedTreasuryServiceLedgerV1 {
            account: ledger,
            body,
        };
        let evidence = TestServiceEvidence {
            realm,
            market,
            record_account,
            record_id,
            policy,
            owner,
            position,
            ledger,
            epoch: Hash32::from_bytes([13; 32]),
            admitted_before: 0,
            settled_before: 0,
            terminal: true,
        };
        (authenticated, derivation, evidence)
    }

    #[test]
    fn service_transition_refuses_stale_counts_identity_swaps_and_false_terminality() {
        let (authenticated, derivation, evidence) = service_transition_fixture();
        let admitted = prepare_treasury_service_admission_v1(
            authenticated,
            derivation,
            &evidence,
        )
        .expect("exact admission");
        assert_eq!(admitted.after().admitted_epoch_count, 1);
        assert_eq!(admitted.after().settled_epoch_count, 0);

        let post = AuthenticatedTreasuryServiceLedgerV1 {
            account: authenticated.account,
            body: admitted.after(),
        };
        assert!(prepare_treasury_service_admission_v1(post, derivation, &evidence).is_err());
        assert!(prepare_treasury_service_settlement_v1(post, derivation, &evidence).is_err());

        let current = TestServiceEvidence {
            admitted_before: 1,
            ..evidence
        };
        let settled = prepare_treasury_service_settlement_v1(post, derivation, &current)
            .expect("exact terminal service");
        assert_eq!(settled.after().admitted_epoch_count, 1);
        assert_eq!(settled.after().settled_epoch_count, 1);

        let wrong_owner = TestServiceEvidence {
            owner: Hash32::from_bytes([14; 32]),
            ..current
        };
        assert!(prepare_treasury_service_settlement_v1(post, derivation, &wrong_owner).is_err());
        let nonterminal = TestServiceEvidence {
            terminal: false,
            ..current
        };
        assert!(prepare_treasury_service_settlement_v1(post, derivation, &nonterminal).is_err());
    }
}
