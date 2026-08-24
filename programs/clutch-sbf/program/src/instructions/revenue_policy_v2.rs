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
use clutch_general_v2_contract::GENERAL_POSITION_FOUNDING_GENERATION_V1;
use clutch_retirement::PositionPurposeV3;
use clutch_solana_layout::registry::RealmRevenueV2Action;
use clutch_solana_layout::revenue::{
    CloseRevenuePolicyRecordV2Payload, InitializeFeeBearingRealmV2Payload,
    RevenuePolicyRecordV2, TreasuryServiceLedgerV1, REVENUE_POLICY_RECORD_BYTES_V2,
    TREASURY_SERVICE_LEDGER_V1_BYTES,
};
use clutch_solana_layout::{
    account_len, canonical_realm_id, Hash32, RealmAccount,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;
use solana_sdk_ids::incinerator;

use super::direct_selection_v3::{
    create_pda_account_full_principal, direct_creation_funding, DIRECT_NEUTRAL_SINK_V3,
};

const TREASURY_SERVICE_ADMISSION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/treasury-service/admit/v1\0";
const TREASURY_SERVICE_SETTLEMENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/treasury-service/settle/v1\0";

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

/// Nonforgeable authentication of one live 0xbb ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedTreasuryServiceLedgerV1 {
    account: Pubkey,
    body: TreasuryServiceLedgerV1,
}

impl AuthenticatedTreasuryServiceLedgerV1 {
    /// Exact physical ledger account.
    pub(crate) const fn account(self) -> Pubkey { self.account }
    /// Exact hostile-decoded body.
    pub(crate) const fn body(self) -> TreasuryServiceLedgerV1 { self.body }
}

/// Authenticate a live 0xbb ledger against the exact immutable Realm/Market
/// derivation. The mutable Position/Replay bodies are authenticated at their
/// own current boundary; this aggregate owns only service conservation.
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
            && body.treasury_position_generation
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

/// Default-refusing authority for exactly one fee-bearing epoch admission.
pub(crate) trait AuthenticatedTreasuryServiceAdmissionV1 {
    fn realm(&self) -> Option<Hash32> { None }
    fn market_instance_v2_id(&self) -> Option<Hash32> { None }
    fn revenue_policy_record_account(&self) -> Option<Pubkey> { None }
    fn revenue_policy_record_v2_id(&self) -> Option<Hash32> { None }
    fn revenue_policy_v2_digest(&self) -> Option<Hash32> { None }
    fn treasury_owner(&self) -> Option<Hash32> { None }
    fn treasury_position_account(&self) -> Option<Pubkey> { None }
    fn treasury_service_ledger_account(&self) -> Option<Pubkey> { None }
    fn epoch_semantic_id(&self) -> Option<Hash32> { None }
    fn admitted_epoch_count_before(&self) -> Option<u64> { None }
    fn settled_epoch_count_before(&self) -> Option<u64> { None }
}

/// Default-refusing authority for exactly one terminal fee-bearing service.
pub(crate) trait AuthenticatedTreasuryServiceSettlementV1:
    AuthenticatedTreasuryServiceAdmissionV1
{
    fn service_is_terminal(&self) -> Option<bool> { None }
}

/// Kind of one exact counted ledger transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TreasuryServiceTransitionKindV1 {
    AdmitEpoch,
    SettleEpoch,
}

/// Private compare-and-write plan for one exact 0xbb transition.
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
    pub(crate) const fn after(self) -> TreasuryServiceLedgerV1 { self.after }
    pub(crate) const fn transition_id(self) -> Hash32 { self.transition_id }
    pub(crate) const fn epoch_semantic_id(self) -> Hash32 { self.epoch_semantic_id }
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
            && evidence.market_instance_v2_id() == Some(derivation.market_instance_v2_id)
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

/// Prepare one exact aggregate admission from private evidence.
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

/// Prepare one exact aggregate settlement from private terminal evidence.
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

/// Compare the actual body, write one prepared transition, and hostile-decode
/// its postimage.
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
}
