// SPDX-License-Identifier: AGPL-3.0-or-later
//! Permanent shared-Market Failure replay owner.
//!
//! Product foundation slot 7 capitalizes this account once. It is neither the
//! legacy occurrence-scoped `0xa3/v1` tombstone nor the reusable interval
//! history. The account remains readable permanently and records exactly one
//! terminal Market-family aggregate. A final Failure-family receipt can then
//! join this authenticated postimage without a recursive receipt identity.

use clutch_product_series::{ContentId as ProductContentId, MarketInstanceV2Id};
use sha2::{Digest, Sha256};

use crate::market_policy_v1::{FailureMarketAccountIdV1, FailureMarketAdmissionStateV1};
use crate::market_runtime_v1::{
    FailureMarketFamilyAggregateReceiptIdV2, FailureMarketFamilyAggregateReceiptV2,
    FailureMarketRuntimeStateCommitmentV1,
};
use crate::{Error, FailurePolicyBindingId, Result};

const FUNDING_DOMAIN_V2: &[u8] = b"dragons-clutch/failure-market-replay-funding/v2";
const STATE_DOMAIN_V2: &[u8] = b"dragons-clutch/failure-market-replay-state/v2";
const TERMINAL_DOMAIN_V2: &[u8] = b"dragons-clutch/failure-market-replay-terminal/v2";
const MAGIC_V2: [u8; 8] = *b"DCFMRPL2";
const VERSION_V2: u16 = 2;
const HEADER_BYTES_V2: usize = 16;
const ID_COUNT_V2: usize = 6;
const AMOUNT_COUNT_V2: usize = 3;

/// Canonical semantic body inside the 256-byte `0xa3/v2` account.
/// The Solana adapter owns the four-byte tag/version/bump frame.
pub const FAILURE_MARKET_REPLAY_BYTES_V2: usize = 252;

macro_rules! replay_id {
    ($name:ident, $docs:literal) => {
        #[doc = $docs]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[repr(transparent)]
        pub struct $name([u8; 32]);

        impl $name {
            /// Construct from digest bytes without claiming authenticity.
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            /// Return exact digest bytes.
            pub const fn bytes(self) -> [u8; 32] {
                self.0
            }
        }
    };
}

replay_id!(
    FailureMarketReplayFundingReceiptIdV2,
    "Typed identity of exact Product-prepaid permanent replay rent."
);
replay_id!(
    FailureMarketReplayStateIdV2,
    "Typed commitment to one complete permanent replay postimage."
);
replay_id!(
    FailureMarketReplayTerminalReceiptIdV2,
    "Typed identity of the exact family aggregate sealed in permanent replay."
);

/// Permanent replay lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FailureMarketReplayPhaseV2 {
    /// Product capitalized the replay; no terminal aggregate exists.
    Pending = 1,
    /// One exact exhaustive family aggregate is permanently sealed.
    Terminal = 2,
}

impl FailureMarketReplayPhaseV2 {
    const fn byte(self) -> u8 {
        match self {
            Self::Pending => 1,
            Self::Terminal => 2,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Pending),
            2 => Ok(Self::Terminal),
            _ => Err(Error::InvalidEnum),
        }
    }
}

/// Exact Product-owned permanent replay capitalization.
///
/// This projection is not authority. The private Product adapter must prove
/// the canonical slot-7 account, live Rent minimum, exact prepaid debit, and
/// postfund balance before admitting it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketReplayFundingFactsV2 {
    /// Shared immutable Failure policy.
    pub failure_policy_binding_id: FailurePolicyBindingId,
    /// Full-width economic Market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Shared Failure/liveness generation.
    pub generation: u64,
    /// Product-private foundation-step receipt.
    pub prepaid_funding_receipt_id: ProductContentId,
    /// Canonical permanent `0xa3/v2` replay account.
    pub replay_account: FailureMarketAccountIdV1,
    /// Immutable economic owner of the permanent rent principal.
    pub permanent_rent_funder: FailureMarketAccountIdV1,
    /// Immutable neutral sink retained for foundation graph separation.
    pub neutral_sink: FailureMarketAccountIdV1,
    /// Exact live Rent minimum for the 256-byte account.
    pub permanent_rent_principal_lamports: u64,
    /// Lamports present before Product capitalization; never principal.
    pub donation_floor_lamports: u64,
    /// Exact post-capitalization balance.
    pub observed_balance_lamports: u64,
}

/// Private Product authority over slot-7 account capitalization.
pub trait AuthenticatedFailureMarketReplayFundingV2 {
    /// Authenticate exact account, Rent, debit, and postfund facts.
    fn authenticate_failure_market_replay_funding(
        &self,
        _expected: FailureMarketReplayFundingFactsV2,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }
}

/// Private-field permanent replay funding receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketReplayFundingReceiptV2 {
    id: FailureMarketReplayFundingReceiptIdV2,
    facts: FailureMarketReplayFundingFactsV2,
}

impl FailureMarketReplayFundingReceiptV2 {
    /// Exact funding receipt identity.
    pub const fn id(self) -> FailureMarketReplayFundingReceiptIdV2 {
        self.id
    }

    /// Complete authenticated foundation facts.
    pub const fn facts(self) -> FailureMarketReplayFundingFactsV2 {
        self.facts
    }
}

/// Permanent replay body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketReplayV2 {
    phase: FailureMarketReplayPhaseV2,
    failure_policy_binding_id: FailurePolicyBindingId,
    market_instance_id: MarketInstanceV2Id,
    prepaid_funding_receipt_id: ProductContentId,
    funding_receipt_id: FailureMarketReplayFundingReceiptIdV2,
    family_aggregate_receipt_id: FailureMarketFamilyAggregateReceiptIdV2,
    runtime_terminal_state_commitment: FailureMarketRuntimeStateCommitmentV1,
    generation: u64,
    permanent_rent_principal_lamports: u64,
    donation_floor_lamports: u64,
}

impl FailureMarketReplayV2 {
    /// Current replay lifecycle.
    pub const fn phase(self) -> FailureMarketReplayPhaseV2 {
        self.phase
    }

    /// Shared immutable Failure policy.
    pub const fn failure_policy_binding_id(self) -> FailurePolicyBindingId {
        self.failure_policy_binding_id
    }

    /// Full-width economic Market.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Shared Failure/liveness generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Exact Product foundation capitalization receipt.
    pub const fn funding_receipt_id(self) -> FailureMarketReplayFundingReceiptIdV2 {
        self.funding_receipt_id
    }

    /// Exact Product slot-7 debit committed at physical creation. Retaining
    /// this preimage component makes later reopening entirely chain-derived.
    pub const fn prepaid_funding_receipt_id(self) -> ProductContentId {
        self.prepaid_funding_receipt_id
    }

    /// Exact terminal family aggregate, or zero while Pending.
    pub const fn family_aggregate_receipt_id(self) -> FailureMarketFamilyAggregateReceiptIdV2 {
        self.family_aggregate_receipt_id
    }

    /// Recovery-closed runtime prestate sealed by the terminal aggregate.
    pub const fn runtime_terminal_state_commitment(self) -> FailureMarketRuntimeStateCommitmentV1 {
        self.runtime_terminal_state_commitment
    }

    /// Exact permanent rent principal admitted at replay creation.
    pub const fn permanent_rent_principal_lamports(self) -> u64 {
        self.permanent_rent_principal_lamports
    }

    /// Unsolicited lamports observed before replay capitalization.
    pub const fn donation_floor_lamports(self) -> u64 {
        self.donation_floor_lamports
    }

    /// Complete semantic state identity.
    pub fn id(self) -> Result<FailureMarketReplayStateIdV2> {
        let mut body = [0; FAILURE_MARKET_REPLAY_BYTES_V2];
        self.encode_into(&mut body)?;
        let mut hasher = Sha256::new();
        hasher.update(STATE_DOMAIN_V2);
        hasher.update(body);
        Ok(FailureMarketReplayStateIdV2::from_bytes(
            hasher.finalize().into(),
        ))
    }

    /// Encode every semantic and reserved byte canonically.
    pub fn encode_into(self, output: &mut [u8; FAILURE_MARKET_REPLAY_BYTES_V2]) -> Result<()> {
        self.validate()?;
        output.fill(0);
        output[..8].copy_from_slice(&MAGIC_V2);
        output[8..10].copy_from_slice(&VERSION_V2.to_le_bytes());
        output[10] = self.phase.byte();
        let mut cursor = HEADER_BYTES_V2;
        for id in [
            self.failure_policy_binding_id.bytes(),
            self.market_instance_id.bytes(),
            self.prepaid_funding_receipt_id.bytes(),
            self.funding_receipt_id.bytes(),
            self.family_aggregate_receipt_id.bytes(),
            self.runtime_terminal_state_commitment.bytes(),
        ] {
            put_id(output, &mut cursor, id)?;
        }
        for amount in [
            self.generation,
            self.permanent_rent_principal_lamports,
            self.donation_floor_lamports,
        ] {
            put_u64(output, &mut cursor, amount)?;
        }
        if output[cursor..].iter().any(|byte| *byte != 0) {
            return Err(Error::WrongLength);
        }
        Ok(())
    }

    /// Hostile-decode only against independently authenticated admission and
    /// Product funding receipts.
    pub fn decode_for_admission(
        input: &[u8; FAILURE_MARKET_REPLAY_BYTES_V2],
        admission: FailureMarketAdmissionStateV1,
        funding: FailureMarketReplayFundingReceiptV2,
    ) -> Result<Self> {
        let value = Self::decode_canonical(input)?;
        value.validate_against(admission, funding)?;
        Ok(value)
    }

    fn decode_canonical(input: &[u8; FAILURE_MARKET_REPLAY_BYTES_V2]) -> Result<Self> {
        if input[..8] != MAGIC_V2 {
            return Err(Error::BadMagic);
        }
        if input[8..10] != VERSION_V2.to_le_bytes() {
            return Err(Error::BadVersion);
        }
        if input[11..HEADER_BYTES_V2].iter().any(|byte| *byte != 0) {
            return Err(Error::NonCanonicalReserved);
        }
        let phase = FailureMarketReplayPhaseV2::decode(input[10])?;
        let mut cursor = HEADER_BYTES_V2;
        let value = Self {
            phase,
            failure_policy_binding_id: FailurePolicyBindingId::from_bytes(take_id(
                input,
                &mut cursor,
            )?),
            market_instance_id: MarketInstanceV2Id::from_bytes(take_id(input, &mut cursor)?),
            prepaid_funding_receipt_id: ProductContentId::from_bytes(take_id(input, &mut cursor)?),
            funding_receipt_id: FailureMarketReplayFundingReceiptIdV2::from_bytes(take_id(
                input,
                &mut cursor,
            )?),
            family_aggregate_receipt_id: FailureMarketFamilyAggregateReceiptIdV2::from_bytes(
                take_id(input, &mut cursor)?,
            ),
            runtime_terminal_state_commitment: FailureMarketRuntimeStateCommitmentV1::from_bytes(
                take_id(input, &mut cursor)?,
            ),
            generation: take_u64(input, &mut cursor)?,
            permanent_rent_principal_lamports: take_u64(input, &mut cursor)?,
            donation_floor_lamports: take_u64(input, &mut cursor)?,
        };
        if input[cursor..].iter().any(|byte| *byte != 0) {
            return Err(Error::NonCanonicalReserved);
        }
        value.validate()?;
        Ok(value)
    }

    fn validate(self) -> Result<()> {
        for id in [
            self.failure_policy_binding_id.bytes(),
            self.market_instance_id.bytes(),
            self.prepaid_funding_receipt_id.bytes(),
            self.funding_receipt_id.bytes(),
        ] {
            require_live(id)?;
        }
        if self.generation == 0 || self.permanent_rent_principal_lamports == 0 {
            return Err(Error::BindingMismatch);
        }
        let aggregate = self.family_aggregate_receipt_id.bytes() != [0; 32];
        let runtime = self.runtime_terminal_state_commitment.bytes() != [0; 32];
        match self.phase {
            FailureMarketReplayPhaseV2::Pending if aggregate || runtime => Err(Error::WrongPhase),
            FailureMarketReplayPhaseV2::Terminal if !(aggregate && runtime) => {
                Err(Error::WrongPhase)
            }
            FailureMarketReplayPhaseV2::Pending | FailureMarketReplayPhaseV2::Terminal => Ok(()),
        }
    }

    fn validate_against(
        self,
        admission: FailureMarketAdmissionStateV1,
        funding: FailureMarketReplayFundingReceiptV2,
    ) -> Result<()> {
        self.validate()?;
        let policy = admission.binding().facts();
        let funded = funding.facts();
        if self.failure_policy_binding_id != admission.binding().id()
            || self.market_instance_id != policy.market_instance_id
            || self.prepaid_funding_receipt_id != funded.prepaid_funding_receipt_id
            || self.generation != policy.generation
            || self.funding_receipt_id != funding.id()
            || funded.failure_policy_binding_id != self.failure_policy_binding_id
            || funded.market_instance_id != self.market_instance_id
            || funded.generation != self.generation
            || funded.permanent_rent_principal_lamports != self.permanent_rent_principal_lamports
            || funded.donation_floor_lamports != self.donation_floor_lamports
        {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }
}

/// Admit Product-prepaid permanent replay and its canonical Pending postimage.
pub fn admit_failure_market_replay_v2<A: AuthenticatedFailureMarketReplayFundingV2 + ?Sized>(
    authority: &A,
    admission: FailureMarketAdmissionStateV1,
    facts: FailureMarketReplayFundingFactsV2,
) -> Result<(FailureMarketReplayV2, FailureMarketReplayFundingReceiptV2)> {
    validate_funding(admission, facts)?;
    authority.authenticate_failure_market_replay_funding(facts)?;
    let mut hasher = Sha256::new();
    hasher.update(FUNDING_DOMAIN_V2);
    hash_funding(&mut hasher, facts);
    let funding_receipt_id =
        FailureMarketReplayFundingReceiptIdV2::from_bytes(hasher.finalize().into());
    require_live(funding_receipt_id.bytes())?;
    let funding = FailureMarketReplayFundingReceiptV2 {
        id: funding_receipt_id,
        facts,
    };
    let replay = FailureMarketReplayV2 {
        phase: FailureMarketReplayPhaseV2::Pending,
        failure_policy_binding_id: admission.binding().id(),
        market_instance_id: admission.binding().facts().market_instance_id,
        prepaid_funding_receipt_id: facts.prepaid_funding_receipt_id,
        funding_receipt_id,
        family_aggregate_receipt_id: FailureMarketFamilyAggregateReceiptIdV2::from_bytes([0; 32]),
        runtime_terminal_state_commitment: FailureMarketRuntimeStateCommitmentV1::from_bytes(
            [0; 32],
        ),
        generation: admission.binding().facts().generation,
        permanent_rent_principal_lamports: facts.permanent_rent_principal_lamports,
        donation_floor_lamports: facts.donation_floor_lamports,
    };
    replay.validate_against(admission, funding)?;
    Ok((replay, funding))
}

/// Reopen the exact Product-capitalization receipt committed by an
/// authenticated permanent replay account.
///
/// These facts are only a hash preimage. Initial creation required Product's
/// private slot-7 receipt, and later code may recover the private-field receipt
/// only when the complete domain-separated preimage equals the funding ID
/// permanently stored by that program-owned replay body.
pub fn reopen_failure_market_replay_funding_v2(
    admission: FailureMarketAdmissionStateV1,
    replay: FailureMarketReplayV2,
    facts: FailureMarketReplayFundingFactsV2,
) -> Result<FailureMarketReplayFundingReceiptV2> {
    validate_funding(admission, facts)?;
    let mut hasher = Sha256::new();
    hasher.update(FUNDING_DOMAIN_V2);
    hash_funding(&mut hasher, facts);
    let id = FailureMarketReplayFundingReceiptIdV2::from_bytes(hasher.finalize().into());
    require_live(id.bytes())?;
    if id != replay.funding_receipt_id
        || facts.failure_policy_binding_id != replay.failure_policy_binding_id
        || facts.market_instance_id != replay.market_instance_id
        || facts.generation != replay.generation
        || facts.permanent_rent_principal_lamports != replay.permanent_rent_principal_lamports
        || facts.donation_floor_lamports != replay.donation_floor_lamports
    {
        return Err(Error::BindingMismatch);
    }
    let funding = FailureMarketReplayFundingReceiptV2 { id, facts };
    replay.validate_against(admission, funding)?;
    Ok(funding)
}

/// Hostile-decode permanent replay and reopen its exact capitalization
/// receipt from a content preimage in one semantic-owner operation.
pub fn decode_and_reopen_failure_market_replay_v2(
    input: &[u8; FAILURE_MARKET_REPLAY_BYTES_V2],
    admission: FailureMarketAdmissionStateV1,
    facts: FailureMarketReplayFundingFactsV2,
) -> Result<(FailureMarketReplayV2, FailureMarketReplayFundingReceiptV2)> {
    let replay = FailureMarketReplayV2::decode_canonical(input)?;
    let funding = reopen_failure_market_replay_funding_v2(admission, replay, facts)?;
    Ok((replay, funding))
}

/// Hostile-decode permanent replay and reconstruct its Product funding
/// receipt entirely from persisted replay/admission state and the physical
/// replay coordinate. No caller-supplied funding preimage participates.
pub fn decode_and_reopen_failure_market_replay_from_chain_v2(
    input: &[u8; FAILURE_MARKET_REPLAY_BYTES_V2],
    admission: FailureMarketAdmissionStateV1,
    replay_account: FailureMarketAccountIdV1,
) -> Result<(FailureMarketReplayV2, FailureMarketReplayFundingReceiptV2)> {
    let replay = FailureMarketReplayV2::decode_canonical(input)?;
    let root_funding = admission.root_funding().facts();
    let policy = admission.binding().facts();
    let observed_balance_lamports = replay
        .permanent_rent_principal_lamports
        .checked_add(replay.donation_floor_lamports)
        .ok_or(Error::BindingMismatch)?;
    let facts = FailureMarketReplayFundingFactsV2 {
        failure_policy_binding_id: admission.binding().id(),
        market_instance_id: policy.market_instance_id,
        generation: policy.generation,
        prepaid_funding_receipt_id: replay.prepaid_funding_receipt_id,
        replay_account,
        permanent_rent_funder: root_funding.rent_payer,
        neutral_sink: FailureMarketAccountIdV1::from_bytes(policy.neutral_sink.bytes()),
        permanent_rent_principal_lamports: replay.permanent_rent_principal_lamports,
        donation_floor_lamports: replay.donation_floor_lamports,
        observed_balance_lamports,
    };
    let funding = reopen_failure_market_replay_funding_v2(admission, replay, facts)?;
    Ok((replay, funding))
}

/// Expected exact permanent replay terminalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketReplayTerminalFactsV2 {
    /// Complete Pending replay prestate.
    pub replay_before: FailureMarketReplayStateIdV2,
    /// Canonical permanent replay account.
    pub replay_account: FailureMarketAccountIdV1,
    /// Exact Product foundation funding receipt.
    pub funding_receipt_id: FailureMarketReplayFundingReceiptIdV2,
    /// Exact pre-replay family aggregate.
    pub family_aggregate_receipt_id: FailureMarketFamilyAggregateReceiptIdV2,
    /// Complete Recovery-closed runtime prestate.
    pub runtime_terminal_state_commitment: FailureMarketRuntimeStateCommitmentV1,
    /// Complete Terminal replay poststate.
    pub replay_after: FailureMarketReplayStateIdV2,
}

/// Private authority over the live roots, Idle cell, and unsealed history.
pub trait AuthenticatedFailureMarketReplayTerminalV2 {
    /// Authenticate the aggregate and exact same-call replay write.
    fn authenticate_failure_market_replay_terminal(
        &self,
        _expected: FailureMarketReplayTerminalFactsV2,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }
}

/// Private-field receipt for the permanent replay terminal postimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketReplayTerminalReceiptV2 {
    id: FailureMarketReplayTerminalReceiptIdV2,
    facts: FailureMarketReplayTerminalFactsV2,
}

impl FailureMarketReplayTerminalReceiptV2 {
    /// Exact terminal replay receipt identity.
    pub const fn id(self) -> FailureMarketReplayTerminalReceiptIdV2 {
        self.id
    }

    /// Complete authenticated replay facts.
    pub const fn facts(self) -> FailureMarketReplayTerminalFactsV2 {
        self.facts
    }
}

/// One stale-checked permanent replay transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketReplayPlanV2 {
    before: FailureMarketReplayV2,
    after: FailureMarketReplayV2,
}

impl FailureMarketReplayPlanV2 {
    /// Complete terminal replay poststate.
    pub const fn resulting_replay(self) -> FailureMarketReplayV2 {
        self.after
    }
}

impl FailureMarketReplayV2 {
    /// Commit the exact Pending-to-Terminal transition once.
    pub fn commit_plan(&mut self, plan: FailureMarketReplayPlanV2) -> Result<()> {
        self.validate()?;
        if *self != plan.before {
            return Err(Error::StalePlan);
        }
        plan.after.validate()?;
        *self = plan.after;
        Ok(())
    }
}

/// Seal one exact pre-replay aggregate into the permanent replay.
pub fn plan_terminalize_failure_market_replay_v2<
    A: AuthenticatedFailureMarketReplayTerminalV2 + ?Sized,
>(
    authority: &A,
    replay: FailureMarketReplayV2,
    admission: FailureMarketAdmissionStateV1,
    funding: FailureMarketReplayFundingReceiptV2,
    aggregate: FailureMarketFamilyAggregateReceiptV2,
) -> Result<(
    FailureMarketReplayPlanV2,
    FailureMarketReplayTerminalReceiptV2,
)> {
    replay.validate_against(admission, funding)?;
    if replay.phase != FailureMarketReplayPhaseV2::Pending {
        return Err(Error::WrongPhase);
    }
    let aggregate_facts = aggregate.facts();
    let funded = funding.facts();
    if aggregate_facts.failure_policy_binding_id != replay.failure_policy_binding_id
        || aggregate_facts.market_instance_id != replay.market_instance_id
        || aggregate_facts.generation != replay.generation
        || aggregate_facts.admission_state_id != admission.id()?
        || funded.replay_account == aggregate_facts.admission_root_account_id
        || funded.replay_account == aggregate_facts.runtime_root_account_id
        || funded.replay_account == aggregate_facts.interval_work_account_id
        || funded.replay_account == aggregate_facts.interval_history_account_id
    {
        return Err(Error::BindingMismatch);
    }
    let replay_before = replay.id()?;
    let mut after = replay;
    after.phase = FailureMarketReplayPhaseV2::Terminal;
    after.family_aggregate_receipt_id = aggregate.id();
    after.runtime_terminal_state_commitment = aggregate_facts.runtime_before;
    after.validate_against(admission, funding)?;
    let replay_after = after.id()?;
    let facts = FailureMarketReplayTerminalFactsV2 {
        replay_before,
        replay_account: funded.replay_account,
        funding_receipt_id: funding.id(),
        family_aggregate_receipt_id: aggregate.id(),
        runtime_terminal_state_commitment: aggregate_facts.runtime_before,
        replay_after,
    };
    authority.authenticate_failure_market_replay_terminal(facts)?;
    let mut hasher = Sha256::new();
    hasher.update(TERMINAL_DOMAIN_V2);
    hasher.update(facts.replay_before.bytes());
    hasher.update(facts.replay_account.bytes());
    hasher.update(facts.funding_receipt_id.bytes());
    hasher.update(facts.family_aggregate_receipt_id.bytes());
    hasher.update(facts.runtime_terminal_state_commitment.bytes());
    hasher.update(facts.replay_after.bytes());
    let id = FailureMarketReplayTerminalReceiptIdV2::from_bytes(hasher.finalize().into());
    require_live(id.bytes())?;
    Ok((
        FailureMarketReplayPlanV2 {
            before: replay,
            after,
        },
        FailureMarketReplayTerminalReceiptV2 { id, facts },
    ))
}

fn validate_funding(
    admission: FailureMarketAdmissionStateV1,
    facts: FailureMarketReplayFundingFactsV2,
) -> Result<()> {
    let policy = admission.binding().facts();
    let expected_balance = facts
        .permanent_rent_principal_lamports
        .checked_add(facts.donation_floor_lamports)
        .ok_or(Error::BindingMismatch)?;
    for id in [
        facts.failure_policy_binding_id.bytes(),
        facts.market_instance_id.bytes(),
        facts.prepaid_funding_receipt_id.bytes(),
        facts.replay_account.bytes(),
        facts.permanent_rent_funder.bytes(),
        facts.neutral_sink.bytes(),
    ] {
        require_live(id)?;
    }
    if facts.failure_policy_binding_id != admission.binding().id()
        || facts.market_instance_id != policy.market_instance_id
        || facts.generation != policy.generation
        || facts.permanent_rent_principal_lamports == 0
        || facts.observed_balance_lamports != expected_balance
        || facts.replay_account == facts.permanent_rent_funder
        || facts.replay_account == facts.neutral_sink
        || facts.permanent_rent_funder == facts.neutral_sink
        || facts.replay_account == admission.root_funding().facts().root_account_id
        || facts.replay_account.bytes() == policy.recovery_state_id.bytes()
        || facts.replay_account.bytes() == policy.recovery_compartment_account_id.bytes()
    {
        return Err(Error::BindingMismatch);
    }
    Ok(())
}

fn hash_funding(hasher: &mut Sha256, facts: FailureMarketReplayFundingFactsV2) {
    hasher.update(facts.failure_policy_binding_id.bytes());
    hasher.update(facts.market_instance_id.bytes());
    hasher.update(facts.generation.to_le_bytes());
    hasher.update(facts.prepaid_funding_receipt_id.bytes());
    hasher.update(facts.replay_account.bytes());
    hasher.update(facts.permanent_rent_funder.bytes());
    hasher.update(facts.neutral_sink.bytes());
    hasher.update(facts.permanent_rent_principal_lamports.to_le_bytes());
    hasher.update(facts.donation_floor_lamports.to_le_bytes());
    hasher.update(facts.observed_balance_lamports.to_le_bytes());
}

fn require_live(bytes: [u8; 32]) -> Result<()> {
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(Error::BindingMismatch);
    }
    Ok(())
}

fn put_id(output: &mut [u8], cursor: &mut usize, value: [u8; 32]) -> Result<()> {
    let end = cursor.checked_add(32).ok_or(Error::WrongLength)?;
    output
        .get_mut(*cursor..end)
        .ok_or(Error::WrongLength)?
        .copy_from_slice(&value);
    *cursor = end;
    Ok(())
}

fn put_u64(output: &mut [u8], cursor: &mut usize, value: u64) -> Result<()> {
    let end = cursor.checked_add(8).ok_or(Error::WrongLength)?;
    output
        .get_mut(*cursor..end)
        .ok_or(Error::WrongLength)?
        .copy_from_slice(&value.to_le_bytes());
    *cursor = end;
    Ok(())
}

fn take_id(input: &[u8], cursor: &mut usize) -> Result<[u8; 32]> {
    let end = cursor.checked_add(32).ok_or(Error::WrongLength)?;
    let mut value = [0; 32];
    value.copy_from_slice(input.get(*cursor..end).ok_or(Error::WrongLength)?);
    *cursor = end;
    Ok(value)
}

fn take_u64(input: &[u8], cursor: &mut usize) -> Result<u64> {
    let end = cursor.checked_add(8).ok_or(Error::WrongLength)?;
    let mut bytes = [0; 8];
    bytes.copy_from_slice(input.get(*cursor..end).ok_or(Error::WrongLength)?);
    *cursor = end;
    Ok(u64::from_le_bytes(bytes))
}

const _: () = {
    assert!(
        HEADER_BYTES_V2 + ID_COUNT_V2 * 32 + AMOUNT_COUNT_V2 * 8 <= FAILURE_MARKET_REPLAY_BYTES_V2
    );
};

#[cfg(test)]
mod tests {
    use super::*;

    fn pending() -> FailureMarketReplayV2 {
        FailureMarketReplayV2 {
            phase: FailureMarketReplayPhaseV2::Pending,
            failure_policy_binding_id: FailurePolicyBindingId::from_bytes([1; 32]),
            market_instance_id: MarketInstanceV2Id::from_bytes([2; 32]),
            prepaid_funding_receipt_id: ProductContentId::from_bytes([3; 32]),
            funding_receipt_id: FailureMarketReplayFundingReceiptIdV2::from_bytes([4; 32]),
            family_aggregate_receipt_id: FailureMarketFamilyAggregateReceiptIdV2::from_bytes(
                [0; 32],
            ),
            runtime_terminal_state_commitment: FailureMarketRuntimeStateCommitmentV1::from_bytes(
                [0; 32],
            ),
            generation: 5,
            permanent_rent_principal_lamports: 6,
            donation_floor_lamports: 7,
        }
    }

    #[test]
    fn terminal_replay_is_one_shot_and_commits_every_reserved_byte() {
        let pending = pending();
        let mut pending_body = [0; FAILURE_MARKET_REPLAY_BYTES_V2];
        pending.encode_into(&mut pending_body).unwrap();
        assert!(pending_body[232..].iter().all(|byte| *byte == 0));
        assert_eq!(
            FailureMarketReplayPhaseV2::decode(0),
            Err(Error::InvalidEnum)
        );

        let mut terminal = pending;
        terminal.phase = FailureMarketReplayPhaseV2::Terminal;
        terminal.family_aggregate_receipt_id =
            FailureMarketFamilyAggregateReceiptIdV2::from_bytes([8; 32]);
        terminal.runtime_terminal_state_commitment =
            FailureMarketRuntimeStateCommitmentV1::from_bytes([9; 32]);
        assert_ne!(pending.id().unwrap(), terminal.id().unwrap());

        let plan = FailureMarketReplayPlanV2 {
            before: pending,
            after: terminal,
        };
        let mut current = pending;
        current.commit_plan(plan).unwrap();
        assert_eq!(current, terminal);
        assert_eq!(current.commit_plan(plan), Err(Error::StalePlan));

        let mut incomplete = terminal;
        incomplete.runtime_terminal_state_commitment =
            FailureMarketRuntimeStateCommitmentV1::from_bytes([0; 32]);
        assert_eq!(incomplete.validate(), Err(Error::WrongPhase));
    }
}
