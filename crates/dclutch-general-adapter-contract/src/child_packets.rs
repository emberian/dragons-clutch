//! Canonical Claims and Custody packets for General settlement effects.
//!
//! This module defines no private child wire. It builds the exact ABI owned by
//! the selected Claims and Custody roles, binds both to one General-owned plan
//! digest, and hostile-verifies the immediate receipts and physical
//! postconditions before a caller may commit the General cursor.

use dclutch_claims_svm::{
    CLAIMS_PLAN_HEADER_BYTES_V1, CLAIMS_RECEIPT_BYTES_V1, CallerRole as ClaimsCallerRole,
    ClaimsAction, ClaimsPlanV1, ClaimsReceiptV1, NO_POSITION_REVISION,
};
use dclutch_custody_contract::{
    CUSTODY_RECEIPT_BYTES_V1, CUSTODY_REQUEST_BYTES_V1, CompartmentV1, ContextV1, CustodyReceiptV1,
    CustodyRequestV1, OperationV1,
};
use dclutch_release_set_contract::ExecutionRoleV1;
use sha2::{Digest, Sha256};

use crate::{
    AggregateReplayContextV1, Error as GeneralError, GeneralChildEffectV1, GeneralChildPlanV2,
    MAX_OUTCOMES, QuoteSurplusRouteV2, RowReplayContextV1,
};

/// Maximum exact Claims request width under the current measured SBF profile.
pub const GENERAL_MAX_CLAIMS_REQUEST_BYTES_V2: usize =
    CLAIMS_PLAN_HEADER_BYTES_V1 + 8 * MAX_OUTCOMES;

/// Stable refusal from child-packet construction or receipt verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildPacketError {
    /// General replay coordinates or quantity shape refused.
    General,
    /// Claims request or receipt bytes refused.
    Claims,
    /// Custody request or receipt bytes refused.
    Custody,
    /// A required physical account, owner, program, or digest was zero.
    Coordinate,
    /// A receipt came from another program or described another poststate.
    ReceiptMismatch,
    /// Checked integer conversion or revision advance failed.
    Arithmetic,
}

impl From<GeneralError> for ChildPacketError {
    fn from(_: GeneralError) -> Self {
        Self::General
    }
}

/// Result alias for canonical General child packets.
pub type ChildPacketResult<T> = core::result::Result<T, ChildPacketError>;

/// Physical Claims coordinates observed before one effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsResourcesV2 {
    /// General settlement Position owner identity.
    pub settlement_owner: [u8; 32],
    /// Current Claims Market revision.
    pub market_revision: u64,
    /// Current row-owner Position revision, or absent sentinel when unused.
    pub owner_position_revision: u64,
    /// Current settlement Position revision, or absent sentinel when unused.
    pub settlement_position_revision: u64,
}

/// Physical Custody coordinates observed before one effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CustodyResourcesV2 {
    /// Immutable Realm content identity.
    pub realm: [u8; 32],
    /// Current Trading program selected by Registry.
    pub trading_program: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Exact source token account.
    pub source: [u8; 32],
    /// Exact destination token account.
    pub destination: [u8; 32],
    /// External source owner, zero for a Custody vault.
    pub source_owner: [u8; 32],
    /// External destination owner, zero for a Custody vault.
    pub destination_owner: [u8; 32],
    /// Custody source-vault context, zero for External.
    pub source_vault_context: [u8; 32],
    /// Custody destination-vault context, zero for External.
    pub destination_vault_context: [u8; 32],
    /// Exact Realm collateral Mint.
    pub mint: [u8; 32],
    /// Exact Realm Token or Token-2022 program.
    pub token_program: [u8; 32],
    /// Current Custody replay revision.
    pub replay_revision: u64,
    /// Ordered transfer coordinate inside this outer effect.
    pub transfer_index: u16,
}

/// Fixed-capacity canonical Claims request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsPacketV2 {
    bytes: [u8; GENERAL_MAX_CLAIMS_REQUEST_BYTES_V2],
    len: usize,
}

impl ClaimsPacketV2 {
    /// Borrow the exact runtime-width request bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    /// SHA-256 of the exact Claims request bytes.
    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        Sha256::digest(self.bytes()).into()
    }

    /// Hostile-decode the exact canonical request.
    pub fn plan(&self) -> ChildPacketResult<ClaimsPlanV1<'_>> {
        ClaimsPlanV1::decode(self.bytes()).map_err(|_| ChildPacketError::Claims)
    }
}

/// Exact canonical Custody request and its digest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CustodyPacketV2 {
    request: CustodyRequestV1,
    bytes: [u8; CUSTODY_REQUEST_BYTES_V1],
}

impl CustodyPacketV2 {
    /// Borrow the exact Custody request bytes.
    #[must_use]
    pub const fn bytes(&self) -> &[u8; CUSTODY_REQUEST_BYTES_V1] {
        &self.bytes
    }

    /// Return the hostile-validated canonical request.
    #[must_use]
    pub const fn request(self) -> CustodyRequestV1 {
        self.request
    }

    /// SHA-256 of the exact Custody request bytes.
    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        Sha256::digest(self.bytes).into()
    }
}

/// Exact child packets required by one General effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralChildPacketsV2 {
    /// Optional Claims mutation.
    pub claims: Option<ClaimsPacketV2>,
    /// Optional Custody mutation.
    pub custody: Option<CustodyPacketV2>,
    /// General-owned request digest bound into every active child.
    pub parent_request_digest: [u8; 32],
}

/// Expected Claims receipt and exact resource revisions after CPI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpectedClaimsPostV2 {
    /// Registry-selected Claims program which must produce return data.
    pub claims_program: [u8; 32],
    /// Exact post-CPI Claims Market revision.
    pub market_revision: u64,
    /// Exact post-CPI source Position revision or absent sentinel.
    pub source_revision: u64,
    /// Exact post-CPI destination Position revision or absent sentinel.
    pub destination_revision: u64,
    /// Exact Claims-derived payout; zero for General settlement actions.
    pub payout: u64,
    /// SHA-256 of exact Market and participating Position poststate bytes.
    pub resource_digest: [u8; 32],
}

/// Expected Custody receipt and exact token/replay postconditions after CPI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpectedCustodyPostV2 {
    /// Registry-selected Custody program which must produce return data.
    pub custody_program: [u8; 32],
    /// Source token amount before CPI.
    pub source_before: u64,
    /// Source token amount after CPI.
    pub source_after: u64,
    /// Destination token amount before CPI.
    pub destination_before: u64,
    /// Destination token amount after CPI.
    pub destination_after: u64,
    /// SHA-256 of exact token and replay poststate.
    pub poststate_commitment: [u8; 32],
    /// SHA-256 of exact committed Custody replay bytes.
    pub replay_state_digest: [u8; 32],
}

/// Build one row-scoped collection or distribution packet set.
#[allow(clippy::too_many_arguments)]
pub fn build_row_packets_v2(
    effect: GeneralChildEffectV1,
    context: RowReplayContextV1,
    outcome_count: u8,
    quantities: &[u64; MAX_OUTCOMES],
    claims: ClaimsResourcesV2,
    custody: Option<CustodyResourcesV2>,
) -> ChildPacketResult<GeneralChildPacketsV2> {
    match effect {
        GeneralChildEffectV1::CollectClaims | GeneralChildEffectV1::DistributeClaims => {
            if custody.is_some() {
                return Err(ChildPacketError::Coordinate);
            }
            build_row_claims_packets_v2(effect, context, outcome_count, quantities, claims)
        }
        GeneralChildEffectV1::CollectCollateral | GeneralChildEffectV1::DistributeCollateral => {
            build_row_custody_packets_v2(
                effect,
                context,
                outcome_count,
                quantities,
                custody.ok_or(ChildPacketError::Coordinate)?,
            )
        }
        _ => Err(ChildPacketError::General),
    }
}

#[inline(never)]
fn build_row_claims_packets_v2(
    effect: GeneralChildEffectV1,
    context: RowReplayContextV1,
    outcome_count: u8,
    quantities: &[u64; MAX_OUTCOMES],
    claims: ClaimsResourcesV2,
) -> ChildPacketResult<GeneralChildPacketsV2> {
    let tail = encode_quantities(outcome_count, quantities)?;
    let active_tail = &tail[..usize::from(outcome_count) * 8];
    let parent =
        GeneralChildPlanV2::new_row(effect, context, u32::from(outcome_count), active_tail)?
            .digest()?;
    let claims_packet = match effect {
        GeneralChildEffectV1::CollectClaims => build_claims_packet(
            ClaimsAction::TransferNative,
            context,
            outcome_count,
            active_tail,
            context.owner_id,
            claims.settlement_owner,
            claims.market_revision,
            claims.owner_position_revision,
            claims.settlement_position_revision,
            parent,
        )?,
        GeneralChildEffectV1::DistributeClaims => build_claims_packet(
            ClaimsAction::TransferNative,
            context,
            outcome_count,
            active_tail,
            claims.settlement_owner,
            context.owner_id,
            claims.market_revision,
            claims.settlement_position_revision,
            claims.owner_position_revision,
            parent,
        )?,
        _ => return Err(ChildPacketError::General),
    };
    Ok(GeneralChildPacketsV2 {
        claims: Some(claims_packet),
        custody: None,
        parent_request_digest: parent,
    })
}

#[inline(never)]
fn build_row_custody_packets_v2(
    effect: GeneralChildEffectV1,
    context: RowReplayContextV1,
    outcome_count: u8,
    quantities: &[u64; MAX_OUTCOMES],
    resources: CustodyResourcesV2,
) -> ChildPacketResult<GeneralChildPacketsV2> {
    if usize::from(outcome_count) > MAX_OUTCOMES
        || outcome_count == 0
        || quantities[1..].iter().any(|quantity| *quantity != 0)
    {
        return Err(ChildPacketError::Coordinate);
    }
    let route_valid = if effect == GeneralChildEffectV1::CollectCollateral {
        resources.source_owner == context.owner_id
            && is_zero(&resources.source_vault_context)
            && is_zero(&resources.destination_owner)
            && resources.destination_vault_context == context.candidate_id
    } else {
        is_zero(&resources.source_owner)
            && resources.source_vault_context == context.candidate_id
            && resources.destination_owner == context.owner_id
            && is_zero(&resources.destination_vault_context)
    };
    if !route_valid {
        return Err(ChildPacketError::Coordinate);
    }
    let quantity = quantities[0].to_le_bytes();
    let parent = GeneralChildPlanV2::new_row(effect, context, 1, &quantity)?.digest()?;
    let custody = build_custody_packet(
        context.execution.release_set_id,
        context.execution.market_id,
        context.candidate_id,
        context.order_id,
        context.order_nonce,
        context.page_index,
        u32::from(context.execution_index),
        quantities[0],
        resources,
        if effect == GeneralChildEffectV1::CollectCollateral {
            CompartmentV1::External
        } else {
            CompartmentV1::Settlement
        },
        if effect == GeneralChildEffectV1::CollectCollateral {
            CompartmentV1::Settlement
        } else {
            CompartmentV1::External
        },
        parent,
    )?;
    Ok(GeneralChildPacketsV2 {
        claims: None,
        custody: Some(custody),
        parent_request_digest: parent,
    })
}

/// Build the Claims and Custody packets for the sole complete-set operation.
#[allow(clippy::too_many_arguments)]
pub fn build_materialize_packets_v2(
    mint: bool,
    context: AggregateReplayContextV1,
    outcome_count: u8,
    quantity: u64,
    claims: ClaimsResourcesV2,
    custody: CustodyResourcesV2,
) -> ChildPacketResult<GeneralChildPacketsV2> {
    let custody_route_valid = if mint {
        is_zero(&custody.source_owner)
            && custody.source_vault_context == context.candidate_id
            && is_zero(&custody.destination_owner)
            && custody.destination_vault_context == context.execution.market_id
    } else {
        is_zero(&custody.source_owner)
            && custody.source_vault_context == context.execution.market_id
            && is_zero(&custody.destination_owner)
            && custody.destination_vault_context == context.candidate_id
    };
    if quantity == 0 || !custody_route_valid {
        return Err(ChildPacketError::Coordinate);
    }
    let mut quantities = [0; MAX_OUTCOMES];
    quantities[..usize::from(outcome_count)].fill(quantity);
    let tail = encode_quantities(outcome_count, &quantities)?;
    let active_tail = &tail[..usize::from(outcome_count) * 8];
    let effect = if mint {
        GeneralChildEffectV1::MintCompleteSet
    } else {
        GeneralChildEffectV1::MergeCompleteSet
    };
    let parent =
        GeneralChildPlanV2::new_aggregate(effect, context, u32::from(outcome_count), active_tail)?
            .digest()?;
    let row = RowReplayContextV1 {
        execution: context.execution,
        candidate_id: context.candidate_id,
        owner_id: claims.settlement_owner,
        order_id: context.candidate_id,
        revision: context.revision,
        order_nonce: 0,
        page_index: 0,
        execution_index: 0,
    };
    let claims_packet = build_claims_packet(
        if mint {
            ClaimsAction::MintCompleteSet
        } else {
            ClaimsAction::MergeCompleteSet
        },
        row,
        outcome_count,
        active_tail,
        if mint {
            [0; 32]
        } else {
            claims.settlement_owner
        },
        if mint {
            claims.settlement_owner
        } else {
            [0; 32]
        },
        claims.market_revision,
        if mint {
            NO_POSITION_REVISION
        } else {
            claims.settlement_position_revision
        },
        if mint {
            claims.settlement_position_revision
        } else {
            NO_POSITION_REVISION
        },
        parent,
    )?;
    let custody_packet = build_custody_packet(
        context.execution.release_set_id,
        context.execution.market_id,
        context.candidate_id,
        context.candidate_id,
        0,
        0,
        0,
        quantity,
        custody,
        if mint {
            CompartmentV1::Settlement
        } else {
            CompartmentV1::HoardPrincipal
        },
        if mint {
            CompartmentV1::HoardPrincipal
        } else {
            CompartmentV1::Settlement
        },
        parent,
    )?;
    Ok(GeneralChildPacketsV2 {
        claims: Some(claims_packet),
        custody: Some(custody_packet),
        parent_request_digest: parent,
    })
}

/// Build the exact terminal quote-surplus Custody packet.
pub fn build_surplus_packet_v2(
    context: AggregateReplayContextV1,
    quantity: u64,
    route: QuoteSurplusRouteV2,
    custody: CustodyResourcesV2,
) -> ChildPacketResult<GeneralChildPacketsV2> {
    if quantity == 0
        || custody.destination != route.destination_account
        || !is_zero(&custody.source_owner)
        || custody.source_vault_context != context.candidate_id
        || custody.destination_owner != route.beneficiary
        || !is_zero(&custody.destination_vault_context)
    {
        return Err(ChildPacketError::Coordinate);
    }
    let tail = quantity.to_le_bytes();
    let parent = GeneralChildPlanV2::new_surplus(context, &tail, route)?.digest()?;
    let custody_packet = build_custody_packet(
        context.execution.release_set_id,
        context.execution.market_id,
        context.candidate_id,
        [0; 32],
        0,
        0,
        0,
        quantity,
        custody,
        CompartmentV1::Settlement,
        CompartmentV1::External,
        parent,
    )?;
    Ok(GeneralChildPacketsV2 {
        claims: None,
        custody: Some(custody_packet),
        parent_request_digest: parent,
    })
}

/// Verify exact Claims return data and caller-observed poststate.
pub fn verify_claims_receipt_v2(
    packet: &ClaimsPacketV2,
    producer: [u8; 32],
    receipt_bytes: &[u8],
    expected: ExpectedClaimsPostV2,
) -> ChildPacketResult<()> {
    if receipt_bytes.len() != CLAIMS_RECEIPT_BYTES_V1
        || producer != expected.claims_program
        || is_zero(&producer)
        || is_zero(&expected.resource_digest)
    {
        return Err(ChildPacketError::ReceiptMismatch);
    }
    let plan = packet.plan()?;
    let receipt = ClaimsReceiptV1::decode(receipt_bytes).map_err(|_| ChildPacketError::Claims)?;
    if receipt.caller_role() != ClaimsCallerRole::Trading
        || receipt.action() != plan.action()
        || receipt.release_set_id() != plan.release_set_id()
        || receipt.market() != plan.market()
        || receipt.request_id() != plan.request_id()
        || receipt.packet_digest() != packet.digest()
        || receipt.claims_program() != producer
        || receipt.pre_market_revision() != plan.expected_market_revision()
        || receipt.post_market_revision() != expected.market_revision
        || receipt.post_source_revision() != expected.source_revision
        || receipt.post_destination_revision() != expected.destination_revision
        || receipt.payout() != expected.payout
        || receipt.post_resource_digest() != expected.resource_digest
    {
        return Err(ChildPacketError::ReceiptMismatch);
    }
    Ok(())
}

/// Verify exact Custody return data and caller-observed token/replay poststate.
pub fn verify_custody_receipt_v2(
    packet: CustodyPacketV2,
    producer: [u8; 32],
    receipt_bytes: &[u8],
    expected: ExpectedCustodyPostV2,
) -> ChildPacketResult<()> {
    if receipt_bytes.len() != CUSTODY_RECEIPT_BYTES_V1
        || producer != expected.custody_program
        || is_zero(&producer)
        || is_zero(&expected.poststate_commitment)
        || is_zero(&expected.replay_state_digest)
    {
        return Err(ChildPacketError::ReceiptMismatch);
    }
    let receipt = CustodyReceiptV1::decode(receipt_bytes).map_err(|_| ChildPacketError::Custody)?;
    receipt
        .verify_for(
            packet.request(),
            packet.digest(),
            expected.replay_state_digest,
        )
        .map_err(|_| ChildPacketError::Custody)?;
    if receipt.evidence.source_before != expected.source_before
        || receipt.evidence.source_after != expected.source_after
        || receipt.evidence.destination_before != expected.destination_before
        || receipt.evidence.destination_after != expected.destination_after
        || receipt.evidence.poststate_commitment != expected.poststate_commitment
        || receipt.evidence.replay_state_digest != expected.replay_state_digest
    {
        return Err(ChildPacketError::ReceiptMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_claims_packet(
    action: ClaimsAction,
    context: RowReplayContextV1,
    outcome_count: u8,
    quantities: &[u8],
    source_owner: [u8; 32],
    destination_owner: [u8; 32],
    market_revision: u64,
    source_revision: u64,
    destination_revision: u64,
    request_id: [u8; 32],
) -> ChildPacketResult<ClaimsPacketV2> {
    let plan = ClaimsPlanV1::new(
        action,
        ClaimsCallerRole::Trading,
        context.execution.release_set_id,
        context.execution.market_id,
        request_id,
        source_owner,
        destination_owner,
        market_revision,
        source_revision,
        destination_revision,
        u32::from(outcome_count),
        quantities,
    )
    .map_err(|_| ChildPacketError::Claims)?;
    let len = CLAIMS_PLAN_HEADER_BYTES_V1
        .checked_add(quantities.len())
        .ok_or(ChildPacketError::Arithmetic)?;
    let mut bytes = [0; GENERAL_MAX_CLAIMS_REQUEST_BYTES_V2];
    plan.encode_into(bytes.get_mut(..len).ok_or(ChildPacketError::Arithmetic)?)
        .map_err(|_| ChildPacketError::Claims)?;
    Ok(ClaimsPacketV2 { bytes, len })
}

#[allow(clippy::too_many_arguments)]
fn build_custody_packet(
    release_set: [u8; 32],
    market: [u8; 32],
    candidate: [u8; 32],
    order: [u8; 32],
    order_nonce: u64,
    page_index: u32,
    execution_index: u32,
    amount: u64,
    resources: CustodyResourcesV2,
    source_compartment: CompartmentV1,
    destination_compartment: CompartmentV1,
    parent_request_digest: [u8; 32],
) -> ChildPacketResult<CustodyPacketV2> {
    let request = CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: ExecutionRoleV1::Trading,
        source_compartment,
        destination_compartment,
        release_set,
        market,
        realm: resources.realm,
        context: candidate,
        caller_program: resources.trading_program,
        semantic: ContextV1 {
            candidate,
            source_owner: resources.source_owner,
            destination_owner: resources.destination_owner,
            order,
            parent_request_digest,
            order_nonce,
            generation: resources.generation,
            page_index,
            execution_index,
            transfer_index: resources.transfer_index,
        },
        source: resources.source,
        destination: resources.destination,
        source_vault_context: resources.source_vault_context,
        destination_vault_context: resources.destination_vault_context,
        mint: resources.mint,
        token_program: resources.token_program,
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: resources.replay_revision,
        resulting_revision: resources
            .replay_revision
            .checked_add(1)
            .ok_or(ChildPacketError::Arithmetic)?,
        amount,
        rent_lamports: 0,
    };
    let bytes = request.to_bytes().map_err(|_| ChildPacketError::Custody)?;
    Ok(CustodyPacketV2 { request, bytes })
}

fn encode_quantities(
    outcome_count: u8,
    quantities: &[u64; MAX_OUTCOMES],
) -> ChildPacketResult<[u8; 8 * MAX_OUTCOMES]> {
    let count = usize::from(outcome_count);
    if count == 0 || count > MAX_OUTCOMES || quantities[count..].iter().any(|value| *value != 0) {
        return Err(ChildPacketError::Coordinate);
    }
    let mut bytes = [0; 8 * MAX_OUTCOMES];
    for (index, quantity) in quantities.iter().take(count).enumerate() {
        let offset = index.checked_mul(8).ok_or(ChildPacketError::Arithmetic)?;
        bytes[offset..offset + 8].copy_from_slice(&quantity.to_le_bytes());
    }
    Ok(bytes)
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_custody_contract::ReceiptEvidenceV1;

    fn id(byte: u8) -> [u8; 32] {
        let mut value = [byte; 32];
        value[31] = byte.wrapping_add(1);
        value
    }

    fn row() -> RowReplayContextV1 {
        RowReplayContextV1 {
            execution: crate::ExecutionContextV1 {
                market_id: id(1),
                release_set_id: id(2),
            },
            candidate_id: id(3),
            owner_id: id(4),
            order_id: id(5),
            revision: 7,
            order_nonce: 11,
            page_index: 2,
            execution_index: 1,
        }
    }

    fn claims() -> ClaimsResourcesV2 {
        ClaimsResourcesV2 {
            settlement_owner: id(6),
            market_revision: 10,
            owner_position_revision: 20,
            settlement_position_revision: 30,
        }
    }

    fn custody(source_external: bool, destination_external: bool) -> CustodyResourcesV2 {
        CustodyResourcesV2 {
            realm: id(8),
            trading_program: id(9),
            generation: 4,
            source: id(10),
            destination: id(11),
            source_owner: if source_external { id(4) } else { [0; 32] },
            destination_owner: if destination_external { id(4) } else { [0; 32] },
            source_vault_context: if source_external { [0; 32] } else { id(3) },
            destination_vault_context: if destination_external { [0; 32] } else { id(3) },
            mint: id(14),
            token_program: id(15),
            replay_revision: 5,
            transfer_index: 1,
        }
    }

    #[test]
    fn runtime_width_sixteen_claims_packet_is_exact_and_parent_bound() {
        let quantities = [7; MAX_OUTCOMES];
        let packets = build_row_packets_v2(
            GeneralChildEffectV1::CollectClaims,
            row(),
            16,
            &quantities,
            claims(),
            None,
        )
        .expect("packets");
        let claims_packet = packets.claims.expect("claims");
        assert_eq!(
            claims_packet.bytes().len(),
            GENERAL_MAX_CLAIMS_REQUEST_BYTES_V2
        );
        assert_eq!(
            claims_packet.plan().expect("plan").request_id(),
            packets.parent_request_digest
        );
        assert!(packets.custody.is_none());
    }

    #[test]
    fn materialize_packets_bind_settlement_to_hoard_custody_direction() {
        let context = AggregateReplayContextV1 {
            execution: row().execution,
            candidate_id: row().candidate_id,
            revision: 9,
        };
        let mut resources = custody(false, false);
        resources.destination_vault_context = row().execution.market_id;
        let packets = build_materialize_packets_v2(true, context, 2, 1, claims(), resources)
            .expect("materialize packets");
        let request = packets.custody.expect("custody").request();
        assert_eq!(request.source_compartment, CompartmentV1::Settlement);
        assert_eq!(
            request.destination_compartment,
            CompartmentV1::HoardPrincipal
        );
    }

    #[test]
    fn distinct_external_owner_collateral_packet_round_trips_and_receipt_checks() {
        let mut quantities = [0; MAX_OUTCOMES];
        quantities[0] = 19;
        let packets = build_row_packets_v2(
            GeneralChildEffectV1::CollectCollateral,
            row(),
            2,
            &quantities,
            claims(),
            Some(custody(true, false)),
        )
        .expect("packets");
        let packet = packets.custody.expect("custody");
        let request = packet.request();
        assert_eq!(request.semantic.source_owner, row().owner_id);
        assert_eq!(request.source_compartment, CompartmentV1::External);
        assert_eq!(request.destination_compartment, CompartmentV1::Settlement);
        let evidence = ReceiptEvidenceV1 {
            source_before: 30,
            source_after: 11,
            destination_before: 2,
            destination_after: 21,
            poststate_commitment: id(20),
            replay_state_digest: id(21),
        };
        let receipt = CustodyReceiptV1::new(request, packet.digest(), evidence)
            .expect("receipt")
            .to_bytes()
            .expect("bytes");
        verify_custody_receipt_v2(
            packet,
            id(22),
            &receipt,
            ExpectedCustodyPostV2 {
                custody_program: id(22),
                source_before: 30,
                source_after: 11,
                destination_before: 2,
                destination_after: 21,
                poststate_commitment: id(20),
                replay_state_digest: id(21),
            },
        )
        .expect("verified");
    }

    #[test]
    fn substituted_producer_or_poststate_refuses() {
        let mut quantities = [0; MAX_OUTCOMES];
        quantities[0] = 19;
        let packet = build_row_packets_v2(
            GeneralChildEffectV1::DistributeCollateral,
            row(),
            2,
            &quantities,
            claims(),
            Some(custody(false, true)),
        )
        .expect("packets")
        .custody
        .expect("custody");
        let evidence = ReceiptEvidenceV1 {
            source_before: 30,
            source_after: 11,
            destination_before: 2,
            destination_after: 21,
            poststate_commitment: id(20),
            replay_state_digest: id(21),
        };
        let receipt = CustodyReceiptV1::new(packet.request(), packet.digest(), evidence)
            .expect("receipt")
            .to_bytes()
            .expect("bytes");
        let expected = ExpectedCustodyPostV2 {
            custody_program: id(22),
            source_before: 30,
            source_after: 11,
            destination_before: 2,
            destination_after: 21,
            poststate_commitment: id(20),
            replay_state_digest: id(21),
        };
        assert_eq!(
            verify_custody_receipt_v2(packet, id(23), &receipt, expected),
            Err(ChildPacketError::ReceiptMismatch)
        );
        let mut wrong = expected;
        wrong.destination_after = 22;
        assert_eq!(
            verify_custody_receipt_v2(packet, id(22), &receipt, wrong),
            Err(ChildPacketError::ReceiptMismatch)
        );
    }
}
