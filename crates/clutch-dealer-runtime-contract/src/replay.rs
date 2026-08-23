// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dealer-specific transition replay without a parallel asset truth.
//!
//! The Replay owns only transition ordering and the last accepted intent. It
//! binds the authoritative Dealer State account and the canonical Position V3
//! account, but never duplicates State phase/generation, Position balances,
//! an active Lease, or transient Pot custody. Runtime account ownership, PDA
//! derivation, signature checks, and receipt parsing remain adapter duties.

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    DealerRuntimeActionV1, DeletableRentOwnerV1, Error, FixedCodec, Id, Result,
    DELETABLE_RENT_OWNER_BYTES,
};

/// Proposed global account tag, intentionally not allocated in the registry yet.
pub const DEALER_FACILITY_REPLAY_PROPOSED_ACCOUNT_TAG_V1: u8 = 0x97;
/// Proposed account schema version, intentionally not runtime-routable yet.
pub const DEALER_FACILITY_REPLAY_PROPOSED_ACCOUNT_VERSION_V1: u8 = 1;
/// Local canonical body magic; this is not the proposed global account tag.
pub const DEALER_FACILITY_REPLAY_MAGIC_V1: [u8; 8] = *b"DCDRPLY1";
/// Exact local semantic-body version.
pub const DEALER_FACILITY_REPLAY_VERSION_V1: u16 = 1;
/// Exact founding ordinal expected by the first transition intent.
pub const DEALER_FACILITY_REPLAY_FOUNDING_ORDINAL_V1: u64 = 0;
/// Exact bytes in one canonical Dealer Facility Replay body.
pub const DEALER_FACILITY_REPLAY_BYTES_V1: usize =
    HEADER_BYTES + (6 * 32) + 8 + DELETABLE_RENT_OWNER_BYTES;
/// Content domain for one canonical Replay body.
pub const DEALER_FACILITY_REPLAY_CONTENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-facility-replay/v1\0";
/// PDA domain for the unique Replay companion.
pub const DEALER_FACILITY_REPLAY_PDA_PREFIX_V1: &[u8] = b"dealer-replay-v1";

/// Local magic for a transition intent committed by the Replay.
pub const DEALER_TRANSITION_INTENT_MAGIC_V1: [u8; 8] = *b"DCDTRNI1";
/// Exact local transition-intent version.
pub const DEALER_TRANSITION_INTENT_VERSION_V1: u16 = 1;
/// Exact bytes in one canonical transition intent.
pub const DEALER_TRANSITION_INTENT_BYTES_V1: usize = HEADER_BYTES + (9 * 32) + 8 + 8;
/// Content domain for one canonical transition intent.
pub const DEALER_TRANSITION_INTENT_CONTENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-transition-intent/v1\0";

const _: () = assert!(DEALER_FACILITY_REPLAY_BYTES_V1 == 292);
const _: () = assert!(DEALER_TRANSITION_INTENT_BYTES_V1 == 316);
const _: () = assert!(DEALER_FACILITY_REPLAY_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);
const _: () = assert!(DEALER_TRANSITION_INTENT_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);

/// Canonical Dealer replay body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFacilityReplayV1 {
    /// Immutable Dealer policy identity.
    pub policy_id: Id,
    /// Immutable facility identity.
    pub facility_id: Id,
    /// Full MarketInstanceV2 identity.
    pub market_instance_v2_id: Id,
    /// Exact authoritative Dealer State V2 account.
    pub dealer_state_account_id: Id,
    /// Exact canonical Position V3 account.
    pub facility_position_account_id: Id,
    /// Ordinal the next accepted transition must carry.
    pub next_transition_ordinal: u64,
    /// Last accepted transition-intent identity, or zero only at founding.
    pub last_transition_intent_id: Id,
    /// Independently funded, lamport-only deletion owner.
    pub rent: DeletableRentOwnerV1,
}

impl DealerFacilityReplayV1 {
    /// Validate full-width joins, founding shape, and lamport-only rent ownership.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            self.policy_id,
            self.facility_id,
            self.market_instance_v2_id,
            self.dealer_state_account_id,
            self.facility_position_account_id,
        ] {
            identity.validate_live()?;
        }
        if self.dealer_state_account_id == self.facility_position_account_id
            || (self.next_transition_ordinal == DEALER_FACILITY_REPLAY_FOUNDING_ORDINAL_V1)
                != self.last_transition_intent_id.is_zero()
        {
            return Err(Error::InvalidParameter);
        }
        self.rent.validate()
    }

    /// Construct the unique founding state for an authenticated account graph.
    pub fn founding(
        policy_id: Id,
        facility_id: Id,
        market_instance_v2_id: Id,
        dealer_state_account_id: Id,
        facility_position_account_id: Id,
        rent: DeletableRentOwnerV1,
    ) -> Result<Self> {
        let replay = Self {
            policy_id,
            facility_id,
            market_instance_v2_id,
            dealer_state_account_id,
            facility_position_account_id,
            next_transition_ordinal: DEALER_FACILITY_REPLAY_FOUNDING_ORDINAL_V1,
            last_transition_intent_id: Id::ZERO,
            rent,
        };
        replay.validate()?;
        Ok(replay)
    }

    /// Canonical semantic identity of this exact Replay body.
    pub fn replay_id(&self) -> Result<Id> {
        let id = self.content_id(DEALER_FACILITY_REPLAY_CONTENT_DOMAIN_V1)?;
        id.validate_live()?;
        Ok(id)
    }

    /// Exact PDA seed facts; the adapter derives and authenticates the address.
    pub const fn pda_seeds(&self) -> DealerFacilityReplayPdaSeedsV1 {
        DealerFacilityReplayPdaSeedsV1 {
            facility_id: self.facility_id,
            dealer_state_account_id: self.dealer_state_account_id,
            facility_position_account_id: self.facility_position_account_id,
        }
    }

    /// Prepare an atomic transition without mutating the replay projection.
    pub fn prepare_transition(
        &self,
        account_binding: DealerReplayAccountBindingV1,
        intent: DealerTransitionIntentV1,
    ) -> Result<PreparedDealerReplayTransitionV1> {
        self.validate()?;
        account_binding.validate()?;
        intent.validate()?;
        let replay_pre_id = self.replay_id()?;
        if account_binding.position_replay_account_id != account_binding.replay_account_id
            || intent.replay_account_id != account_binding.replay_account_id
            || intent.replay_pre_id != replay_pre_id
            || intent.expected_ordinal != self.next_transition_ordinal
        {
            return Err(Error::MismatchedBinding);
        }
        let intent_id = intent.intent_id()?;
        let next_transition_ordinal = self
            .next_transition_ordinal
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let replay_post = Self {
            next_transition_ordinal,
            last_transition_intent_id: intent_id,
            ..*self
        };
        replay_post.validate()?;
        Ok(PreparedDealerReplayTransitionV1 {
            replay_account_id: account_binding.replay_account_id,
            replay_pre_id,
            replay_post,
            intent,
            intent_id,
        })
    }
}

impl FixedCodec for DealerFacilityReplayV1 {
    const ENCODED_LEN: usize = DEALER_FACILITY_REPLAY_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &DEALER_FACILITY_REPLAY_MAGIC_V1,
            DEALER_FACILITY_REPLAY_VERSION_V1,
        );
        for identity in [
            self.policy_id,
            self.facility_id,
            self.market_instance_v2_id,
            self.dealer_state_account_id,
            self.facility_position_account_id,
            self.last_transition_intent_id,
        ] {
            writer.id(identity);
        }
        writer.u64(self.next_transition_ordinal);
        self.rent.encode_body(&mut writer);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &DEALER_FACILITY_REPLAY_MAGIC_V1,
            DEALER_FACILITY_REPLAY_VERSION_V1,
        )?;
        let value = Self {
            policy_id: reader.id(),
            facility_id: reader.id(),
            market_instance_v2_id: reader.id(),
            dealer_state_account_id: reader.id(),
            facility_position_account_id: reader.id(),
            last_transition_intent_id: reader.id(),
            next_transition_ordinal: reader.u64(),
            rent: DeletableRentOwnerV1::decode_body(&mut reader),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Exact canonical Replay PDA seed facts, excluding the program id and bump.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFacilityReplayPdaSeedsV1 {
    /// Immutable facility seed.
    pub facility_id: Id,
    /// Authoritative State-account seed.
    pub dealer_state_account_id: Id,
    /// Canonical Position V3 account seed.
    pub facility_position_account_id: Id,
}

/// Runtime-authenticated actual Replay account joined to Position V3.
///
/// Fields remain public because this pure carrier is not an authentication
/// capability. The adapter must derive the Replay PDA and parse Position V3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerReplayAccountBindingV1 {
    /// Actual Replay account presented to the instruction.
    pub replay_account_id: Id,
    /// Exact Replay account retained by the authenticated Position V3 body.
    pub position_replay_account_id: Id,
}

impl DealerReplayAccountBindingV1 {
    fn validate(self) -> Result<()> {
        self.replay_account_id.validate_live()?;
        self.position_replay_account_id.validate_live()?;
        if self.replay_account_id != self.position_replay_account_id {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }
}

/// How one transition obtains execution liveness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DealerTransitionLivenessModeV1 {
    /// The transaction caller pays; no external receipt may be supplied.
    CallerFunded = 0,
    /// The exact canonical external-runtime receipt is consumed.
    ExternalReceipt = 1,
}

impl DealerTransitionLivenessModeV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::CallerFunded),
            1 => Ok(Self::ExternalReceipt),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// Canonical transition intent whose identity becomes Replay's last intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerTransitionIntentV1 {
    /// Actual, PDA-authenticated Replay account.
    pub replay_account_id: Id,
    /// Semantic identity of the exact Replay pre-state.
    pub replay_pre_id: Id,
    /// Exact Dealer State content identity before the transition.
    pub state_pre_content_id: Id,
    /// Exact Dealer State content identity after the transition.
    pub state_post_content_id: Id,
    /// Exact canonical Position V3 semantic identity before the transition.
    pub position_pre_semantic_id: Id,
    /// Exact canonical Position V3 semantic identity after the transition.
    pub position_post_semantic_id: Id,
    /// Exact external liveness receipt, or zero only for caller-funded actions.
    pub liveness_receipt_semantic_id: Id,
    /// Exact canonical fee settlement/abort receipt, or zero when inapplicable.
    pub fee_receipt_semantic_id: Id,
    /// Content identity of the complete exact asset-transfer bundle.
    pub asset_transfer_bundle_id: Id,
    /// Ordinal consumed from Replay.
    pub expected_ordinal: u64,
    /// Exact Dealer action being committed.
    pub action: DealerRuntimeActionV1,
    /// Caller-funded or exact external-receipt liveness.
    pub liveness_mode: DealerTransitionLivenessModeV1,
}

impl DealerTransitionIntentV1 {
    /// Validate identity presence and action-specific receipt requirements.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            self.replay_account_id,
            self.replay_pre_id,
            self.state_pre_content_id,
            self.state_post_content_id,
            self.position_pre_semantic_id,
            self.position_post_semantic_id,
            self.asset_transfer_bundle_id,
        ] {
            identity.validate_live()?;
        }
        let external_receipt_present = !self.liveness_receipt_semantic_id.is_zero();
        match self.liveness_mode {
            DealerTransitionLivenessModeV1::CallerFunded => {
                if external_receipt_present {
                    return Err(Error::MismatchedBinding);
                }
            }
            DealerTransitionLivenessModeV1::ExternalReceipt => {
                self.liveness_receipt_semantic_id.validate_live()?;
            }
        }
        match liveness_policy(self.action)? {
            DealerActionLivenessPolicyV1::CallerOnly => {
                if self.liveness_mode != DealerTransitionLivenessModeV1::CallerFunded {
                    return Err(Error::MismatchedBinding);
                }
            }
            DealerActionLivenessPolicyV1::ExternalOnly => {
                if self.liveness_mode != DealerTransitionLivenessModeV1::ExternalReceipt {
                    return Err(Error::MismatchedBinding);
                }
            }
            DealerActionLivenessPolicyV1::Either => {}
        }
        if action_requires_fee_receipt(self.action) {
            self.fee_receipt_semantic_id.validate_live()?;
        } else if !self.fee_receipt_semantic_id.is_zero() {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Canonical transition-intent identity.
    pub fn intent_id(&self) -> Result<Id> {
        let id = self.content_id(DEALER_TRANSITION_INTENT_CONTENT_DOMAIN_V1)?;
        id.validate_live()?;
        Ok(id)
    }
}

impl FixedCodec for DealerTransitionIntentV1 {
    const ENCODED_LEN: usize = DEALER_TRANSITION_INTENT_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &DEALER_TRANSITION_INTENT_MAGIC_V1,
            DEALER_TRANSITION_INTENT_VERSION_V1,
        );
        for identity in [
            self.replay_account_id,
            self.replay_pre_id,
            self.state_pre_content_id,
            self.state_post_content_id,
            self.position_pre_semantic_id,
            self.position_post_semantic_id,
            self.liveness_receipt_semantic_id,
            self.fee_receipt_semantic_id,
            self.asset_transfer_bundle_id,
        ] {
            writer.id(identity);
        }
        writer.u64(self.expected_ordinal);
        writer.u8(action_byte(self.action));
        writer.u8(liveness_mode_byte(self.liveness_mode));
        writer.reserved(6);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &DEALER_TRANSITION_INTENT_MAGIC_V1,
            DEALER_TRANSITION_INTENT_VERSION_V1,
        )?;
        let replay_account_id = reader.id();
        let replay_pre_id = reader.id();
        let state_pre_content_id = reader.id();
        let state_post_content_id = reader.id();
        let position_pre_semantic_id = reader.id();
        let position_post_semantic_id = reader.id();
        let liveness_receipt_semantic_id = reader.id();
        let fee_receipt_semantic_id = reader.id();
        let asset_transfer_bundle_id = reader.id();
        let expected_ordinal = reader.u64();
        let action = decode_action(reader.u8())?;
        let liveness_mode = DealerTransitionLivenessModeV1::decode(reader.u8())?;
        reader.reserved(6)?;
        reader.finish()?;
        let value = Self {
            replay_account_id,
            replay_pre_id,
            state_pre_content_id,
            state_post_content_id,
            position_pre_semantic_id,
            position_post_semantic_id,
            liveness_receipt_semantic_id,
            fee_receipt_semantic_id,
            asset_transfer_bundle_id,
            expected_ordinal,
            action,
            liveness_mode,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Opaque prepared replay advance; no field can be forged independently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedDealerReplayTransitionV1 {
    replay_account_id: Id,
    replay_pre_id: Id,
    replay_post: DealerFacilityReplayV1,
    intent: DealerTransitionIntentV1,
    intent_id: Id,
}

impl PreparedDealerReplayTransitionV1 {
    /// Expected canonical Replay post-state.
    pub const fn replay_post(self) -> DealerFacilityReplayV1 {
        self.replay_post
    }

    /// Exact accepted intent identity.
    pub const fn intent_id(self) -> Id {
        self.intent_id
    }
}

/// Runtime-observed atomic postconditions for an accepted transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerTransitionCommitObservationV1 {
    /// Actual mutated Replay account.
    pub replay_account_id: Id,
    /// Semantic identity recomputed from the Replay bytes before mutation.
    pub replay_pre_id: Id,
    /// Reloaded and decoded Replay post-state.
    pub replay_post: DealerFacilityReplayV1,
    /// Exact Dealer State content identity after all writes.
    pub state_post_content_id: Id,
    /// Exact canonical Position V3 semantic identity after all writes.
    pub position_post_semantic_id: Id,
    /// Exact executed asset-transfer receipt binding the complete bundle.
    pub asset_transfer_receipt_id: Id,
    /// Exact consumed liveness receipt, or zero for caller-funded execution.
    pub liveness_receipt_semantic_id: Id,
    /// Exact consumed fee receipt, or zero when inapplicable.
    pub fee_receipt_semantic_id: Id,
}

/// Accept a replay advance only after every State, Position, asset, fee, and
/// liveness postcondition has been reloaded and matched atomically.
pub fn accept_dealer_replay_transition_v1(
    prepared: PreparedDealerReplayTransitionV1,
    observed: DealerTransitionCommitObservationV1,
) -> Result<DealerFacilityReplayV1> {
    observed.replay_post.validate()?;
    if observed.replay_account_id != prepared.replay_account_id
        || observed.replay_pre_id != prepared.replay_pre_id
        || observed.replay_post != prepared.replay_post
        || observed.replay_post.last_transition_intent_id != prepared.intent_id
        || observed.state_post_content_id != prepared.intent.state_post_content_id
        || observed.position_post_semantic_id != prepared.intent.position_post_semantic_id
        || observed.asset_transfer_receipt_id != prepared.intent.asset_transfer_bundle_id
        || observed.liveness_receipt_semantic_id != prepared.intent.liveness_receipt_semantic_id
        || observed.fee_receipt_semantic_id != prepared.intent.fee_receipt_semantic_id
    {
        return Err(Error::MismatchedBinding);
    }
    Ok(observed.replay_post)
}

/// Authenticated terminal joins required before deleting a Facility Replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerReplayTerminalJoinV1 {
    /// Actual Replay account being deleted.
    pub replay_account_id: Id,
    /// Exact Replay account retained by terminal Position V3.
    pub position_replay_account_id: Id,
    /// Exact authoritative Dealer State account.
    pub dealer_state_account_id: Id,
    /// Exact canonical Position V3 account.
    pub facility_position_account_id: Id,
    /// Whether authenticated State V2 is in Retiring.
    pub dealer_state_is_retiring: bool,
    /// Whether authenticated Position V3 has minted its terminal projection.
    pub position_is_terminal: bool,
}

/// Lamport-only Replay close plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerReplayClosePlanV1 {
    /// Replay account deleted to zero lamports/data.
    pub replay_account_id: Id,
    /// Rent payer and sole principal-refund recipient.
    pub payer: Id,
    /// Canonical Realm neutral lamport sink.
    pub neutral_sink: Id,
    /// Exact lamport principal refunded to the payer.
    pub payer_refund_lamports: u64,
    /// Donation floor and every later surplus routed to the neutral sink.
    pub neutral_surplus_lamports: u64,
    /// Expected payer balance after the atomic close.
    pub payer_balance_after: u64,
    /// Expected neutral-sink balance after the atomic close.
    pub neutral_sink_balance_after: u64,
}

/// Plan Replay deletion only after exact State/Position terminal joins.
///
/// This function moves lamports only. Position cash, native Eggs, Hoard
/// principal, fees, and liveness compartments are absent by construction.
pub fn plan_dealer_replay_close_v1(
    replay: DealerFacilityReplayV1,
    join: DealerReplayTerminalJoinV1,
    replay_lamports: u64,
    payer_balance_before: u64,
    neutral_sink_balance_before: u64,
) -> Result<DealerReplayClosePlanV1> {
    replay.validate()?;
    for identity in [
        join.replay_account_id,
        join.position_replay_account_id,
        join.dealer_state_account_id,
        join.facility_position_account_id,
    ] {
        identity.validate_live()?;
    }
    if join.replay_account_id != join.position_replay_account_id
        || join.dealer_state_account_id != replay.dealer_state_account_id
        || join.facility_position_account_id != replay.facility_position_account_id
        || !join.dealer_state_is_retiring
        || !join.position_is_terminal
    {
        return Err(Error::MismatchedBinding);
    }
    let minimum_balance = replay
        .rent
        .refundable_principal
        .checked_add(replay.rent.donation_floor)
        .ok_or(Error::ArithmeticOverflow)?;
    if replay_lamports < minimum_balance {
        return Err(Error::InvalidParameter);
    }
    let neutral_surplus_lamports = replay_lamports
        .checked_sub(replay.rent.refundable_principal)
        .ok_or(Error::ArithmeticOverflow)?;
    let payer_balance_after = payer_balance_before
        .checked_add(replay.rent.refundable_principal)
        .ok_or(Error::ArithmeticOverflow)?;
    let neutral_sink_balance_after = neutral_sink_balance_before
        .checked_add(neutral_surplus_lamports)
        .ok_or(Error::ArithmeticOverflow)?;
    Ok(DealerReplayClosePlanV1 {
        replay_account_id: join.replay_account_id,
        payer: replay.rent.payer,
        neutral_sink: replay.rent.neutral_sink,
        payer_refund_lamports: replay.rent.refundable_principal,
        neutral_surplus_lamports,
        payer_balance_after,
        neutral_sink_balance_after,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DealerActionLivenessPolicyV1 {
    CallerOnly,
    ExternalOnly,
    Either,
}

fn liveness_policy(action: DealerRuntimeActionV1) -> Result<DealerActionLivenessPolicyV1> {
    match action {
        DealerRuntimeActionV1::CreatePolicy => Err(Error::InvalidParameter),
        DealerRuntimeActionV1::Contribute
        | DealerRuntimeActionV1::WithdrawFunding
        | DealerRuntimeActionV1::SponsorHalt => Ok(DealerActionLivenessPolicyV1::CallerOnly),
        DealerRuntimeActionV1::QueueExit => Ok(DealerActionLivenessPolicyV1::Either),
        DealerRuntimeActionV1::Initialize
        | DealerRuntimeActionV1::CreateLpPage
        | DealerRuntimeActionV1::Activate
        | DealerRuntimeActionV1::CancelFunding
        | DealerRuntimeActionV1::RefundCancelledSponsor
        | DealerRuntimeActionV1::BindEpoch
        | DealerRuntimeActionV1::LapseEpoch
        | DealerRuntimeActionV1::SelectLeaseAndBegin
        | DealerRuntimeActionV1::Collect
        | DealerRuntimeActionV1::Deliver
        | DealerRuntimeActionV1::FinalizeSettlement
        | DealerRuntimeActionV1::AbortBeforeCollection
        | DealerRuntimeActionV1::EnterUnwind
        | DealerRuntimeActionV1::TimedClose
        | DealerRuntimeActionV1::Resolve
        | DealerRuntimeActionV1::Claim
        | DealerRuntimeActionV1::Retire => Ok(DealerActionLivenessPolicyV1::ExternalOnly),
    }
}

const fn action_requires_fee_receipt(action: DealerRuntimeActionV1) -> bool {
    matches!(
        action,
        DealerRuntimeActionV1::SelectLeaseAndBegin
            | DealerRuntimeActionV1::Collect
            | DealerRuntimeActionV1::Deliver
            | DealerRuntimeActionV1::FinalizeSettlement
            | DealerRuntimeActionV1::AbortBeforeCollection
    )
}

const fn liveness_mode_byte(mode: DealerTransitionLivenessModeV1) -> u8 {
    match mode {
        DealerTransitionLivenessModeV1::CallerFunded => 0,
        DealerTransitionLivenessModeV1::ExternalReceipt => 1,
    }
}

pub(crate) const fn action_byte(action: DealerRuntimeActionV1) -> u8 {
    match action {
        DealerRuntimeActionV1::CreatePolicy => 0,
        DealerRuntimeActionV1::Initialize => 1,
        DealerRuntimeActionV1::CreateLpPage => 2,
        DealerRuntimeActionV1::Contribute => 3,
        DealerRuntimeActionV1::WithdrawFunding => 4,
        DealerRuntimeActionV1::Activate => 5,
        DealerRuntimeActionV1::CancelFunding => 6,
        DealerRuntimeActionV1::RefundCancelledSponsor => 7,
        DealerRuntimeActionV1::BindEpoch => 8,
        DealerRuntimeActionV1::LapseEpoch => 9,
        DealerRuntimeActionV1::SelectLeaseAndBegin => 10,
        DealerRuntimeActionV1::Collect => 11,
        DealerRuntimeActionV1::Deliver => 12,
        DealerRuntimeActionV1::FinalizeSettlement => 13,
        DealerRuntimeActionV1::AbortBeforeCollection => 14,
        DealerRuntimeActionV1::QueueExit => 15,
        DealerRuntimeActionV1::SponsorHalt => 16,
        DealerRuntimeActionV1::EnterUnwind => 17,
        DealerRuntimeActionV1::TimedClose => 18,
        DealerRuntimeActionV1::Resolve => 19,
        DealerRuntimeActionV1::Claim => 20,
        DealerRuntimeActionV1::Retire => 21,
    }
}

pub(crate) fn decode_action(value: u8) -> Result<DealerRuntimeActionV1> {
    match value {
        0 => Ok(DealerRuntimeActionV1::CreatePolicy),
        1 => Ok(DealerRuntimeActionV1::Initialize),
        2 => Ok(DealerRuntimeActionV1::CreateLpPage),
        3 => Ok(DealerRuntimeActionV1::Contribute),
        4 => Ok(DealerRuntimeActionV1::WithdrawFunding),
        5 => Ok(DealerRuntimeActionV1::Activate),
        6 => Ok(DealerRuntimeActionV1::CancelFunding),
        7 => Ok(DealerRuntimeActionV1::RefundCancelledSponsor),
        8 => Ok(DealerRuntimeActionV1::BindEpoch),
        9 => Ok(DealerRuntimeActionV1::LapseEpoch),
        10 => Ok(DealerRuntimeActionV1::SelectLeaseAndBegin),
        11 => Ok(DealerRuntimeActionV1::Collect),
        12 => Ok(DealerRuntimeActionV1::Deliver),
        13 => Ok(DealerRuntimeActionV1::FinalizeSettlement),
        14 => Ok(DealerRuntimeActionV1::AbortBeforeCollection),
        15 => Ok(DealerRuntimeActionV1::QueueExit),
        16 => Ok(DealerRuntimeActionV1::SponsorHalt),
        17 => Ok(DealerRuntimeActionV1::EnterUnwind),
        18 => Ok(DealerRuntimeActionV1::TimedClose),
        19 => Ok(DealerRuntimeActionV1::Resolve),
        20 => Ok(DealerRuntimeActionV1::Claim),
        21 => Ok(DealerRuntimeActionV1::Retire),
        _ => Err(Error::InvalidParameter),
    }
}
