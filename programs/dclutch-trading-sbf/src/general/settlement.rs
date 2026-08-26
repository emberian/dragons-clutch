//! Atomic General settlement preparation behind authenticated Trading.
//!
//! The functions in this module consume the canonical controller cursor and
//! immutable candidate/page bytes, build exact Claims and Custody requests,
//! and return one complete cursor commit candidate. The SBF account adapter
//! must invoke every active packet, verify its immediate producer/receipt and
//! observed poststate, and only then copy `cursor_after` into account memory.

extern crate alloc;

use alloc::boxed::Box;

use dclutch_core_contract::ContentId;
use dclutch_general_adapter_contract::child_packets::{
    ChildPacketError, ClaimsPacketV2, ClaimsResourcesV2, CustodyPacketV2, CustodyResourcesV2,
    build_materialize_packets_v2, build_row_packets_v2, build_surplus_packet_v2,
};
use dclutch_general_adapter_contract::{
    AggregateReplayContextV1, ChildExecutionError, ExecutionContextV1, GeneralChildEffectV1,
    RowReplayContextV1, SettlementChildrenV1, VerifiedCandidateV1, close, collect_execution,
    distribute_execution, materialize,
};
use dclutch_general_codec::{MAX_OUTCOMES, SETTLEMENT_CURSOR_BYTES};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::pubkey::Pubkey;

/// Stable refusal while preparing one atomic physical settlement action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettlementPreparationErrorV2 {
    /// Candidate/page/cursor semantics refused.
    Controller,
    /// Exact Claims/Custody request construction refused.
    ChildPacket,
    /// More than one request for the same fixed role was projected.
    DuplicateRole,
    /// A required collateral route was absent.
    MissingRoute,
}

impl From<ChildPacketError> for SettlementPreparationErrorV2 {
    fn from(_: ChildPacketError) -> Self {
        Self::ChildPacket
    }
}

/// Exact commit candidate for one General settlement instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedSettlementStepV2 {
    cursor_after: [u8; SETTLEMENT_CURSOR_BYTES],
    claims: Option<ClaimsPacketV2>,
    custody: Option<CustodyPacketV2>,
}

/// Packet-derived Trading signer PDAs required by the active fixed-role children.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementCallerAuthoritiesV2 {
    /// Claims caller signer, absent when this step has no Claims effect.
    pub claims: Option<[u8; 32]>,
    /// Custody caller signer, absent when this step has no Custody effect.
    pub custody: Option<[u8; 32]>,
}

impl PreparedSettlementStepV2 {
    /// Exact General cursor bytes to commit after every child postcondition.
    #[must_use]
    pub const fn cursor_after(self) -> [u8; SETTLEMENT_CURSOR_BYTES] {
        self.cursor_after
    }

    /// Optional exact Claims request.
    #[must_use]
    pub const fn claims(self) -> Option<ClaimsPacketV2> {
        self.claims
    }

    /// Optional exact Custody request.
    #[must_use]
    pub const fn custody(self) -> Option<CustodyPacketV2> {
        self.custody
    }
}

/// Derive the two independent packet-bound Trading caller authorities.
///
/// Claims binds its authority context to the General parent request digest;
/// Custody binds its authority context to the candidate. A single generic
/// General authority cannot authenticate both children and is deliberately
/// not accepted here.
pub fn derive_caller_authorities_v2(
    prepared: PreparedSettlementStepV2,
    trading_program: [u8; 32],
    release_set: [u8; 32],
    market: [u8; 32],
) -> Result<SettlementCallerAuthoritiesV2, SettlementPreparationErrorV2> {
    let program = Pubkey::new_from_array(trading_program);
    let content =
        ContentId::new(release_set).map_err(|_| SettlementPreparationErrorV2::ChildPacket)?;
    let claims = match prepared.claims() {
        Some(packet) => {
            let plan = packet
                .plan()
                .map_err(|_| SettlementPreparationErrorV2::ChildPacket)?;
            if plan.release_set_id() != release_set || plan.market() != market {
                return Err(SettlementPreparationErrorV2::ChildPacket);
            }
            let seeds = CallerAuthoritySeedsV1::new(
                content,
                market,
                ExecutionRoleV1::Trading,
                plan.request_id(),
                packet.digest(),
            )
            .map_err(|_| SettlementPreparationErrorV2::ChildPacket)?;
            Some(
                Pubkey::find_program_address(&seeds.as_slices(), &program)
                    .0
                    .to_bytes(),
            )
        }
        None => None,
    };
    let custody = match prepared.custody() {
        Some(packet) => {
            let request = packet.request();
            if request.release_set != release_set
                || request.market != market
                || request.caller_program != trading_program
            {
                return Err(SettlementPreparationErrorV2::ChildPacket);
            }
            let seeds = CallerAuthoritySeedsV1::new(
                content,
                market,
                ExecutionRoleV1::Trading,
                request.context,
                packet.digest(),
            )
            .map_err(|_| SettlementPreparationErrorV2::ChildPacket)?;
            Some(
                Pubkey::find_program_address(&seeds.as_slices(), &program)
                    .0
                    .to_bytes(),
            )
        }
        None => None,
    };
    if claims.is_some() && claims == custody {
        return Err(SettlementPreparationErrorV2::ChildPacket);
    }
    Ok(SettlementCallerAuthoritiesV2 { claims, custody })
}

/// Prepare one exact collection-row transition without mutating source bytes.
pub fn prepare_collect_v2(
    cursor_before: &[u8],
    context: ExecutionContextV1,
    verified: &VerifiedCandidateV1,
    page_bytes: &[u8],
    expected_revision: u64,
    claims: ClaimsResourcesV2,
    custody: Option<CustodyResourcesV2>,
) -> Result<PreparedSettlementStepV2, SettlementPreparationErrorV2> {
    let mut cursor_after = exact_cursor_copy(cursor_before)?;
    let mut recorder = Box::new(PacketRecorderV2::new(claims, custody, None));
    let result = collect_execution(
        &mut cursor_after,
        context,
        verified,
        page_bytes,
        expected_revision,
        recorder.as_mut(),
    );
    recorder.require_controller_success(result)?;
    (*recorder).finish(cursor_after)
}

/// Prepare the sole candidate-wide mint, merge, or no-op transition.
pub fn prepare_materialize_v2(
    cursor_before: &[u8],
    context: ExecutionContextV1,
    verified: &VerifiedCandidateV1,
    expected_revision: u64,
    claims: ClaimsResourcesV2,
    custody: Option<CustodyResourcesV2>,
) -> Result<PreparedSettlementStepV2, SettlementPreparationErrorV2> {
    let mut cursor_after = exact_cursor_copy(cursor_before)?;
    let mut recorder = Box::new(PacketRecorderV2::new(claims, custody, None));
    let result = materialize(
        &mut cursor_after,
        context,
        verified,
        expected_revision,
        recorder.as_mut(),
    );
    recorder.require_controller_success(result)?;
    (*recorder).finish(cursor_after)
}

/// Prepare one exact distribution-row transition without mutating source bytes.
pub fn prepare_distribute_v2(
    cursor_before: &[u8],
    context: ExecutionContextV1,
    verified: &VerifiedCandidateV1,
    page_bytes: &[u8],
    expected_revision: u64,
    claims: ClaimsResourcesV2,
    custody: Option<CustodyResourcesV2>,
) -> Result<PreparedSettlementStepV2, SettlementPreparationErrorV2> {
    let mut cursor_after = exact_cursor_copy(cursor_before)?;
    let mut recorder = Box::new(PacketRecorderV2::new(claims, custody, None));
    let result = distribute_execution(
        &mut cursor_after,
        context,
        verified,
        page_bytes,
        expected_revision,
        recorder.as_mut(),
    );
    recorder.require_controller_success(result)?;
    (*recorder).finish(cursor_after)
}

/// Prepare the terminal zero-inventory transition and optional surplus route.
pub fn prepare_close_v2(
    cursor_before: &[u8],
    context: ExecutionContextV1,
    verified: &VerifiedCandidateV1,
    expected_revision: u64,
    claims: ClaimsResourcesV2,
    custody: Option<CustodyResourcesV2>,
    surplus_route: Option<dclutch_general_adapter_contract::QuoteSurplusRouteV2>,
) -> Result<PreparedSettlementStepV2, SettlementPreparationErrorV2> {
    let mut cursor_after = exact_cursor_copy(cursor_before)?;
    let mut recorder = Box::new(PacketRecorderV2::new(claims, custody, surplus_route));
    let result = close(
        &mut cursor_after,
        context,
        verified,
        expected_revision,
        recorder.as_mut(),
    );
    recorder.require_controller_success(result)?;
    (*recorder).finish(cursor_after)
}

struct PacketRecorderV2 {
    claims_resources: ClaimsResourcesV2,
    custody_resources: Option<CustodyResourcesV2>,
    surplus_route: Option<dclutch_general_adapter_contract::QuoteSurplusRouteV2>,
    claims: Option<ClaimsPacketV2>,
    custody: Option<CustodyPacketV2>,
    error: Option<SettlementPreparationErrorV2>,
}

impl PacketRecorderV2 {
    const fn new(
        claims_resources: ClaimsResourcesV2,
        custody_resources: Option<CustodyResourcesV2>,
        surplus_route: Option<dclutch_general_adapter_contract::QuoteSurplusRouteV2>,
    ) -> Self {
        Self {
            claims_resources,
            custody_resources,
            surplus_route,
            claims: None,
            custody: None,
            error: None,
        }
    }

    fn record(
        &mut self,
        packets: Result<
            dclutch_general_adapter_contract::child_packets::GeneralChildPacketsV2,
            ChildPacketError,
        >,
    ) -> core::result::Result<(), ChildExecutionError> {
        let packets = match packets {
            Ok(value) => value,
            Err(error) => {
                self.error = Some(error.into());
                return Err(ChildExecutionError::Refused);
            }
        };
        if (packets.claims.is_some() && self.claims.is_some())
            || (packets.custody.is_some() && self.custody.is_some())
        {
            self.error = Some(SettlementPreparationErrorV2::DuplicateRole);
            return Err(ChildExecutionError::Refused);
        }
        self.claims = packets.claims.or(self.claims);
        self.custody = packets.custody.or(self.custody);
        Ok(())
    }

    fn require_controller_success(
        &self,
        result: dclutch_general_adapter_contract::Result<()>,
    ) -> Result<(), SettlementPreparationErrorV2> {
        match (result, self.error) {
            (Ok(()), _) => Ok(()),
            (Err(_), Some(error)) => Err(error),
            (Err(_), None) => Err(SettlementPreparationErrorV2::Controller),
        }
    }

    fn finish(
        self,
        cursor_after: [u8; SETTLEMENT_CURSOR_BYTES],
    ) -> Result<PreparedSettlementStepV2, SettlementPreparationErrorV2> {
        if let Some(error) = self.error {
            return Err(error);
        }
        Ok(PreparedSettlementStepV2 {
            cursor_after,
            claims: self.claims,
            custody: self.custody,
        })
    }
}

impl SettlementChildrenV1 for PacketRecorderV2 {
    fn collect_claims(
        &mut self,
        context: RowReplayContextV1,
        outcome_count: u8,
        quantities: &[u64; MAX_OUTCOMES],
    ) -> core::result::Result<(), ChildExecutionError> {
        self.record(build_row_packets_v2(
            GeneralChildEffectV1::CollectClaims,
            context,
            outcome_count,
            quantities,
            self.claims_resources,
            None,
        ))
    }

    fn collect_collateral(
        &mut self,
        context: RowReplayContextV1,
        quantity: u64,
    ) -> core::result::Result<(), ChildExecutionError> {
        let mut quantities = [0; MAX_OUTCOMES];
        quantities[0] = quantity;
        self.record(build_row_packets_v2(
            GeneralChildEffectV1::CollectCollateral,
            context,
            1,
            &quantities,
            self.claims_resources,
            self.custody_resources,
        ))
    }

    fn mint_complete_set(
        &mut self,
        context: AggregateReplayContextV1,
        outcome_count: u8,
        quantity: u64,
    ) -> core::result::Result<(), ChildExecutionError> {
        let custody = match self.custody_resources {
            Some(value) => value,
            None => {
                self.error = Some(SettlementPreparationErrorV2::MissingRoute);
                return Err(ChildExecutionError::Refused);
            }
        };
        self.record(build_materialize_packets_v2(
            true,
            context,
            outcome_count,
            quantity,
            self.claims_resources,
            custody,
        ))
    }

    fn merge_complete_set(
        &mut self,
        context: AggregateReplayContextV1,
        outcome_count: u8,
        quantity: u64,
    ) -> core::result::Result<(), ChildExecutionError> {
        let custody = match self.custody_resources {
            Some(value) => value,
            None => {
                self.error = Some(SettlementPreparationErrorV2::MissingRoute);
                return Err(ChildExecutionError::Refused);
            }
        };
        self.record(build_materialize_packets_v2(
            false,
            context,
            outcome_count,
            quantity,
            self.claims_resources,
            custody,
        ))
    }

    fn distribute_claims(
        &mut self,
        context: RowReplayContextV1,
        outcome_count: u8,
        quantities: &[u64; MAX_OUTCOMES],
    ) -> core::result::Result<(), ChildExecutionError> {
        self.record(build_row_packets_v2(
            GeneralChildEffectV1::DistributeClaims,
            context,
            outcome_count,
            quantities,
            self.claims_resources,
            None,
        ))
    }

    fn distribute_collateral(
        &mut self,
        context: RowReplayContextV1,
        quantity: u64,
    ) -> core::result::Result<(), ChildExecutionError> {
        let mut quantities = [0; MAX_OUTCOMES];
        quantities[0] = quantity;
        self.record(build_row_packets_v2(
            GeneralChildEffectV1::DistributeCollateral,
            context,
            1,
            &quantities,
            self.claims_resources,
            self.custody_resources,
        ))
    }

    fn pay_surplus(
        &mut self,
        context: AggregateReplayContextV1,
        quantity: u64,
    ) -> core::result::Result<(), ChildExecutionError> {
        let custody = match self.custody_resources {
            Some(value) => value,
            None => {
                self.error = Some(SettlementPreparationErrorV2::MissingRoute);
                return Err(ChildExecutionError::Refused);
            }
        };
        let route = match self.surplus_route {
            Some(value) => value,
            None => {
                self.error = Some(SettlementPreparationErrorV2::MissingRoute);
                return Err(ChildExecutionError::Refused);
            }
        };
        self.record(build_surplus_packet_v2(context, quantity, route, custody))
    }
}

fn exact_cursor_copy(
    cursor_before: &[u8],
) -> Result<[u8; SETTLEMENT_CURSOR_BYTES], SettlementPreparationErrorV2> {
    cursor_before
        .try_into()
        .map_err(|_| SettlementPreparationErrorV2::Controller)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_general_adapter_contract::{CandidateVerifierV1, CompleteSetMoveV1};
    use dclutch_general_codec::{
        CandidateV1, ExecutionV1, MAX_EXECUTIONS_PER_PAGE, PAGE_BYTES, PageV1, Phase,
        SettlementCursorV1,
    };

    fn id(byte: u8) -> [u8; 32] {
        let mut value = [byte; 32];
        value[31] = byte.wrapping_add(1);
        value
    }

    fn vector(first: u64, second: u64) -> [u64; MAX_OUTCOMES] {
        let mut value = [0; MAX_OUTCOMES];
        value[0] = first;
        value[1] = second;
        value
    }

    fn fixture() -> (ExecutionContextV1, VerifiedCandidateV1, [u8; PAGE_BYTES]) {
        let candidate = CandidateV1 {
            outcome_count: 2,
            candidate_id: id(1),
            product_id: id(2),
            batch_id: id(3),
            page_count: 1,
            price_scale: 2,
            prices: vector(1, 1),
        };
        let mut rows = [ExecutionV1::EMPTY; MAX_EXECUTIONS_PER_PAGE];
        rows[0] = ExecutionV1 {
            order_id: id(4),
            owner_id: id(5),
            nonce: 7,
            max_lots: 1,
            max_quote_debit_per_lot: 2,
            lots: 1,
            quote_debit: 1,
            quote_credit: 0,
            receive_per_lot: vector(1, 0),
            deliver_per_lot: [0; MAX_OUTCOMES],
        };
        rows[1] = ExecutionV1 {
            order_id: id(6),
            owner_id: id(7),
            nonce: 8,
            max_lots: 1,
            max_quote_debit_per_lot: 2,
            lots: 1,
            quote_debit: 1,
            quote_credit: 0,
            receive_per_lot: vector(0, 1),
            deliver_per_lot: [0; MAX_OUTCOMES],
        };
        let page = PageV1 {
            outcome_count: 2,
            candidate_id: candidate.candidate_id,
            page_index: 0,
            page_count: 1,
            execution_count: 2,
            executions: rows,
        }
        .to_bytes()
        .expect("page");
        let mut verifier = CandidateVerifierV1::begin(candidate);
        verifier.ingest_page(&page).expect("page verify");
        let verified = verifier.finish().expect("candidate verify");
        assert_eq!(verified.complete_set_move, CompleteSetMoveV1::Mint);
        (
            ExecutionContextV1 {
                market_id: id(8),
                release_set_id: id(9),
            },
            verified,
            page,
        )
    }

    fn claims() -> ClaimsResourcesV2 {
        ClaimsResourcesV2 {
            settlement_owner: id(10),
            market_revision: 1,
            owner_position_revision: 2,
            settlement_position_revision: 3,
        }
    }

    fn custody(external_source: bool) -> CustodyResourcesV2 {
        CustodyResourcesV2 {
            realm: id(11),
            trading_program: id(12),
            generation: 1,
            source: id(13),
            destination: id(14),
            source_owner: if external_source { id(5) } else { [0; 32] },
            destination_owner: [0; 32],
            source_vault_context: if external_source { [0; 32] } else { id(1) },
            destination_vault_context: if external_source { id(1) } else { id(8) },
            mint: id(16),
            token_program: id(17),
            replay_revision: 1,
            transfer_index: 0,
        }
    }

    #[test]
    fn cursor_bytes_remain_unchanged_while_exact_packets_are_prepared() {
        let (context, verified, page) = fixture();
        let cursor = SettlementCursorV1 {
            phase: Phase::Collecting,
            outcome_count: 2,
            candidate_id: verified.candidate_id,
            page_count: 1,
            next_page: 0,
            next_execution: 0,
            revision: 0,
            claim_inventory: [0; MAX_OUTCOMES],
            quote_inventory: 0,
            quote_surplus_paid: 0,
        }
        .to_bytes()
        .expect("cursor");
        let before = cursor;
        let prepared = prepare_collect_v2(
            &cursor,
            context,
            &verified,
            &page,
            0,
            claims(),
            Some(custody(true)),
        )
        .expect("prepare");
        assert_eq!(cursor, before);
        assert!(prepared.claims().is_none());
        assert!(prepared.custody().is_some());
        assert_ne!(prepared.cursor_after(), before);
    }

    #[test]
    fn materialization_projects_both_roles_from_one_parent_semantics() {
        let (context, verified, _) = fixture();
        let cursor = SettlementCursorV1 {
            phase: Phase::Materializing,
            outcome_count: 2,
            candidate_id: verified.candidate_id,
            page_count: 1,
            next_page: 0,
            next_execution: 0,
            revision: 2,
            claim_inventory: [0; MAX_OUTCOMES],
            quote_inventory: 2,
            quote_surplus_paid: 0,
        }
        .to_bytes()
        .expect("cursor");
        let prepared = prepare_materialize_v2(
            &cursor,
            context,
            &verified,
            2,
            claims(),
            Some(custody(false)),
        )
        .expect("prepare");
        assert!(prepared.claims().is_some());
        assert!(prepared.custody().is_some());
        let authorities = derive_caller_authorities_v2(
            prepared,
            id(12),
            context.release_set_id,
            context.market_id,
        )
        .expect("authorities");
        assert!(authorities.claims.is_some());
        assert!(authorities.custody.is_some());
        assert_ne!(authorities.claims, authorities.custody);
        let after = SettlementCursorV1::decode(&prepared.cursor_after()).expect("after");
        assert_eq!(after.phase, Phase::Distributing);
        assert_eq!(after.claim_inventory[0], 1);
        assert_eq!(after.claim_inventory[1], 1);
        assert_eq!(after.quote_inventory, 1);
    }

    #[test]
    fn missing_materialization_custody_refuses_without_a_commit_candidate() {
        let (context, verified, _) = fixture();
        let cursor = SettlementCursorV1 {
            phase: Phase::Materializing,
            outcome_count: 2,
            candidate_id: verified.candidate_id,
            page_count: 1,
            next_page: 0,
            next_execution: 0,
            revision: 2,
            claim_inventory: [0; MAX_OUTCOMES],
            quote_inventory: 2,
            quote_surplus_paid: 0,
        }
        .to_bytes()
        .expect("cursor");
        assert_eq!(
            prepare_materialize_v2(&cursor, context, &verified, 2, claims(), None),
            Err(SettlementPreparationErrorV2::MissingRoute)
        );
    }
}
