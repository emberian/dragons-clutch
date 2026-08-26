//! Chain-derived unsigned Dealer request construction.
//!
//! Callers provide one already authenticated snapshot of the canonical
//! Trading root, active/pending Candidate records, and Claims Position. Every
//! optimistic coordinate is copied from that snapshot; user choices contain
//! only economic intent such as side, quantity, or proposed Candidate.

use dclutch_dealer_codec::{
    Action, CandidateView, Error as DealerError, Phase, Policy, Side,
    root_tail::RootTail,
    trading_request::{TRADING_REQUEST_BYTES, TradingRequest},
};

/// Stable refusal from unsigned Dealer construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerOperatorError {
    /// The projected chain state was internally inconsistent.
    InvalidState,
    /// The requested action could not be canonically encoded.
    InvalidChoice,
}

/// Exact unsigned Dealer request bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsignedDealerRequestV2 {
    bytes: [u8; TRADING_REQUEST_BYTES],
}

impl UnsignedDealerRequestV2 {
    /// Borrow the exact request passed after the common hot Trading envelope.
    pub const fn as_bytes(&self) -> &[u8; TRADING_REQUEST_BYTES] {
        &self.bytes
    }

    /// Decode the request for display or independent inspection.
    pub fn decode(self) -> Result<TradingRequest, DealerOperatorError> {
        TradingRequest::decode(&self.bytes).map_err(|_| DealerOperatorError::InvalidState)
    }
}

/// Authenticated chain snapshot used for one request-construction batch.
#[derive(Clone, Copy)]
pub struct DealerChainProjectionV2<'a> {
    /// Immutable selected Dealer policy.
    pub policy: Policy,
    /// Inventory-free mutable root tail.
    pub tail: RootTail,
    /// Exact active Candidate record.
    pub active: CandidateView<'a>,
    /// Exact pending Candidate when the tail has one.
    pub pending: Option<CandidateView<'a>>,
    /// Canonical internal Dealer Claims Position revision.
    pub position_revision: u64,
}

impl DealerChainProjectionV2<'_> {
    fn validate(self) -> Result<(), DealerOperatorError> {
        self.tail
            .to_bytes()
            .map_err(|_| DealerOperatorError::InvalidState)?;
        if self.active.candidate_id != self.tail.active_candidate_id
            || self.active.revision != self.tail.active_revision
            || self.active.outcome_count != self.policy.outcome_count
            || self.policy.market_id == [0; 32]
            || self.policy.dealer_id == [0; 32]
        {
            return Err(DealerOperatorError::InvalidState);
        }
        match (self.tail.pending_candidate_id == [0; 32], self.pending) {
            (true, None) => {}
            (false, Some(pending))
                if pending.candidate_id == self.tail.pending_candidate_id
                    && pending.revision == self.tail.pending_revision => {}
            _ => return Err(DealerOperatorError::InvalidState),
        }
        Ok(())
    }

    fn base(self, action: Action) -> Result<TradingRequest, DealerOperatorError> {
        self.validate()?;
        Ok(TradingRequest {
            action,
            side: Side::TakerBuys,
            outcome: 0,
            expected_state_revision: self.tail.state_revision,
            expected_position_revision: self.position_revision,
            now: 0,
            quantity: 0,
            expected_candidate_id: self.tail.active_candidate_id,
            actor_id: [0; 32],
            replacement_candidate_id: [0; 32],
            expected_candidate_revision: self.tail.active_revision,
        })
    }
}

/// Construct a permissionless exact curve fill.
pub fn build_fill_v2(
    state: DealerChainProjectionV2<'_>,
    side: Side,
    outcome: u8,
    quantity: u64,
    now: u64,
) -> Result<UnsignedDealerRequestV2, DealerOperatorError> {
    if state.tail.phase != Phase::Open
        || usize::from(outcome) >= usize::from(state.policy.outcome_count)
        || quantity == 0
    {
        return Err(DealerOperatorError::InvalidChoice);
    }
    encode(TradingRequest {
        side,
        outcome,
        quantity,
        now,
        ..state.base(Action::Fill)?
    })
}

/// Construct an owner-authorized quote or native-claim liquidity increase.
pub fn build_add_liquidity_v2(
    state: DealerChainProjectionV2<'_>,
    asset_coordinate: u8,
    quantity: u64,
) -> Result<UnsignedDealerRequestV2, DealerOperatorError> {
    build_liquidity(state, Action::AddLiquidity, asset_coordinate, quantity)
}

/// Construct an owner-authorized quote or native-claim liquidity decrease.
pub fn build_remove_liquidity_v2(
    state: DealerChainProjectionV2<'_>,
    asset_coordinate: u8,
    quantity: u64,
) -> Result<UnsignedDealerRequestV2, DealerOperatorError> {
    build_liquidity(state, Action::RemoveLiquidity, asset_coordinate, quantity)
}

fn build_liquidity(
    state: DealerChainProjectionV2<'_>,
    action: Action,
    asset_coordinate: u8,
    quantity: u64,
) -> Result<UnsignedDealerRequestV2, DealerOperatorError> {
    if state.tail.phase != Phase::Open
        || asset_coordinate > state.policy.outcome_count
        || quantity == 0
    {
        return Err(DealerOperatorError::InvalidChoice);
    }
    encode(TradingRequest {
        action,
        outcome: asset_coordinate,
        quantity,
        actor_id: state.policy.dealer_id,
        ..state.base(action)?
    })
}

/// Construct an owner-authorized delayed Candidate replacement schedule.
pub fn build_schedule_replacement_v2(
    state: DealerChainProjectionV2<'_>,
    proposed: CandidateView<'_>,
    now: u64,
) -> Result<UnsignedDealerRequestV2, DealerOperatorError> {
    if state.tail.phase != Phase::Open
        || proposed.outcome_count != state.policy.outcome_count
        || proposed.revision <= state.tail.pending_revision.max(state.tail.active_revision)
        || proposed.candidate_id == [0; 32]
    {
        return Err(DealerOperatorError::InvalidChoice);
    }
    encode(TradingRequest {
        now,
        actor_id: state.policy.dealer_id,
        replacement_candidate_id: proposed.candidate_id,
        ..state.base(Action::ScheduleReplacement)?
    })
}

/// Construct a permissionless activation of the exact pending Candidate.
pub fn build_activate_replacement_v2(
    state: DealerChainProjectionV2<'_>,
    now: u64,
) -> Result<UnsignedDealerRequestV2, DealerOperatorError> {
    let pending = state.pending.ok_or(DealerOperatorError::InvalidState)?;
    encode(TradingRequest {
        now,
        replacement_candidate_id: pending.candidate_id,
        ..state.base(Action::ActivateReplacement)?
    })
}

/// Construct the Core-derived terminal transition request.
pub fn build_enter_terminal_v2(
    state: DealerChainProjectionV2<'_>,
    winner: u8,
) -> Result<UnsignedDealerRequestV2, DealerOperatorError> {
    if state.tail.phase != Phase::Open
        || usize::from(winner) >= usize::from(state.policy.outcome_count)
    {
        return Err(DealerOperatorError::InvalidChoice);
    }
    encode(TradingRequest {
        outcome: winner,
        actor_id: state.policy.market_id,
        ..state.base(Action::EnterTerminal)?
    })
}

/// Construct one permissionless terminal claim redemption/burn.
pub fn build_unwind_v2(
    state: DealerChainProjectionV2<'_>,
    outcome: u8,
    quantity: u64,
) -> Result<UnsignedDealerRequestV2, DealerOperatorError> {
    if state.tail.phase != Phase::Terminal
        || usize::from(outcome) >= usize::from(state.policy.outcome_count)
        || quantity == 0
    {
        return Err(DealerOperatorError::InvalidChoice);
    }
    encode(TradingRequest {
        outcome,
        quantity,
        ..state.base(Action::Unwind)?
    })
}

/// Construct terminal principal/fee/work-refund retirement.
pub fn build_retire_v2(
    state: DealerChainProjectionV2<'_>,
) -> Result<UnsignedDealerRequestV2, DealerOperatorError> {
    if state.tail.phase != Phase::Terminal {
        return Err(DealerOperatorError::InvalidChoice);
    }
    encode(state.base(Action::Retire)?)
}

fn encode(request: TradingRequest) -> Result<UnsignedDealerRequestV2, DealerOperatorError> {
    let bytes = request.to_bytes().map_err(map_dealer_error)?;
    Ok(UnsignedDealerRequestV2 { bytes })
}

fn map_dealer_error(_: DealerError) -> DealerOperatorError {
    DealerOperatorError::InvalidChoice
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_dealer_codec::{
        CANDIDATE_BYTES, CandidateInput, CurveBand, CurveInput, encode_candidate,
    };

    fn policy() -> Policy {
        Policy {
            market_id: [1; 32],
            release_set_id: [2; 32],
            dealer_id: [3; 32],
            fee_recipient_id: [4; 32],
            unwind_recipient_id: [5; 32],
            outcome_count: 2,
            quote_scale: 100,
            fee_numerator: 1,
            fee_denominator: 100,
            minimum_work_funding: 10,
            replacement_delay: 5,
        }
    }

    fn candidate(id: u8, revision: u64, valid_from: u64) -> [u8; CANDIDATE_BYTES] {
        let bids = [CurveBand {
            capacity: 100,
            price_numerator: 40,
        }];
        let asks = [CurveBand {
            capacity: 100,
            price_numerator: 60,
        }];
        let curves = [CurveInput {
            bids: &bids,
            asks: &asks,
        }; 2];
        let mut bytes = [0_u8; CANDIDATE_BYTES];
        encode_candidate(
            &mut bytes,
            CandidateInput {
                candidate_id: [id; 32],
                revision,
                valid_from,
                expires_at: 1_000,
                quote_reserve_floor: 10,
                work_funding: 20,
                work_reward: 1,
                minimum_inventory: &[0, 0],
                maximum_inventory: &[100, 100],
                curves: &curves,
            },
        )
        .expect("candidate");
        bytes
    }

    fn state<'a>(
        active: CandidateView<'a>,
        pending: Option<CandidateView<'a>>,
        phase: Phase,
    ) -> DealerChainProjectionV2<'a> {
        DealerChainProjectionV2 {
            policy: policy(),
            tail: RootTail {
                phase,
                active_candidate_id: active.candidate_id,
                pending_candidate_id: pending.map_or([0; 32], |value| value.candidate_id),
                active_revision: active.revision,
                pending_revision: pending.map_or(0, |value| value.revision),
                state_revision: 7,
                buy_used: [0; dclutch_dealer_codec::MAX_OUTCOMES],
                sell_used: [0; dclutch_dealer_codec::MAX_OUTCOMES],
                fee_base: 0,
                active_work_remaining: 20,
                pending_work_funding: pending.map_or(0, |value| value.work_funding),
            },
            active,
            pending,
            position_revision: 9,
        }
    }

    #[test]
    fn all_open_routes_copy_chain_revisions_and_candidate_coordinates() {
        let active_bytes = candidate(10, 3, 0);
        let next_bytes = candidate(11, 4, 20);
        let active = CandidateView::decode(&active_bytes).expect("active");
        let next = CandidateView::decode(&next_bytes).expect("next");
        let open = state(active, None, Phase::Open);
        for request in [
            build_fill_v2(open, Side::TakerBuys, 1, 5, 10).expect("fill"),
            build_add_liquidity_v2(open, 2, 30).expect("quote add"),
            build_remove_liquidity_v2(open, 0, 4).expect("claim remove"),
            build_schedule_replacement_v2(open, next, 10).expect("schedule"),
            build_enter_terminal_v2(open, 1).expect("terminal"),
        ] {
            let decoded = request.decode().expect("decode");
            assert_eq!(decoded.expected_state_revision, 7);
            assert_eq!(decoded.expected_position_revision, 9);
            assert_eq!(decoded.expected_candidate_id, [10; 32]);
            assert_eq!(decoded.expected_candidate_revision, 3);
        }
        let pending = state(active, Some(next), Phase::Open);
        assert_eq!(
            build_activate_replacement_v2(pending, 20)
                .expect("activate")
                .decode()
                .expect("decode")
                .replacement_candidate_id,
            [11; 32]
        );
    }

    #[test]
    fn terminal_routes_and_hostile_stale_snapshot_shapes_refuse() {
        let active_bytes = candidate(10, 3, 0);
        let active = CandidateView::decode(&active_bytes).expect("active");
        let terminal = state(active, None, Phase::Terminal);
        assert_eq!(
            build_unwind_v2(terminal, 1, 5)
                .expect("unwind")
                .decode()
                .expect("decode")
                .action,
            Action::Unwind
        );
        assert_eq!(
            build_retire_v2(terminal)
                .expect("retire")
                .decode()
                .expect("decode")
                .action,
            Action::Retire
        );
        let mut hostile = state(active, None, Phase::Open);
        hostile.tail.active_revision = 99;
        assert_eq!(
            build_fill_v2(hostile, Side::TakerBuys, 0, 1, 0),
            Err(DealerOperatorError::InvalidState)
        );
    }
}
