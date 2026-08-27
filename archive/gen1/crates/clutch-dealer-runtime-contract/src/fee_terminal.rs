// SPDX-License-Identifier: AGPL-3.0-or-later

//! Read-only Dealer join to the fee runtime's candidate terminal receipt.
//!
//! Fee collection and refund accounting remain owned by the fee runtime and
//! General Position settlement. This module only authenticates that one
//! terminal receipt closes the exact selected fee record already frozen into
//! a V2 Lease. It grants no custody, capitalization, or mutation authority.

use clutch_fee_runtime_contract::terminal::{
    DealerFeeTerminalProjectionV1, FeeTerminalOutcomeV1,
};

use crate::{DealerEpochBindingPhaseV2, DealerEpochBindingV2, DealerLeaseV2, Error, Id, Result};

/// Dealer-authenticated terminal evidence for one exact leased candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFeeTerminalJoinV1 {
    /// Fee-owned read-only projection; never a Dealer semantic owner.
    pub terminal: DealerFeeTerminalProjectionV1,
    /// Exact terminal receipt identity converted into the Dealer identity type.
    pub terminal_receipt_id: Id,
    /// Selected fee-record account frozen into the Lease.
    pub selected_fee_record_account_id: Id,
    /// Immutable selected fee-record semantic identity retained by the Lease.
    /// The fee terminal does not duplicate it; the authenticated Lease remains
    /// its sole Dealer-side owner.
    pub selected_fee_record_semantic_id: Id,
    /// Final settlement candidate shared by Epoch, Lease, and fee terminal.
    pub settlement_candidate_id: Id,
    /// Exact owner-netted revenue policy shared by Lease and fee terminal.
    pub fee_revenue_policy_id: Id,
    /// Settled or aborted outcome owned by the fee terminal receipt.
    pub outcome: FeeTerminalOutcomeV1,
}

impl DealerFeeTerminalJoinV1 {
    /// A terminal fee receipt never capitalizes Dealer liveness.
    pub const fn available_liveness_lamports(&self) -> u64 {
        0
    }

    /// Hoard principal is not present in, or authorized by, this join.
    pub const fn available_hoard_atoms(&self) -> u64 {
        0
    }

    /// Collected or projected fee value is not a Dealer funding source.
    pub const fn available_fee_funding_atoms(&self) -> u64 {
        0
    }
}

/// Bind fee-owned terminal evidence to the authoritative V2 Epoch and Lease.
///
/// The selected record's semantic ID is deliberately read only from `lease`:
/// it was authenticated at lease selection and is not reintroduced as a
/// caller-supplied terminal summary.
pub fn bind_dealer_fee_terminal_v1(
    terminal: DealerFeeTerminalProjectionV1,
    epoch: &DealerEpochBindingV2,
    lease: &DealerLeaseV2,
) -> Result<DealerFeeTerminalJoinV1> {
    epoch.validate()?;
    lease.validate()?;
    let terminal_receipt_id = dealer_id(terminal.terminal_receipt);
    let fee_record_account_id = dealer_id(terminal.fee_record);
    let settlement_candidate_id = dealer_id(terminal.settlement_candidate);
    let fee_revenue_policy_id = dealer_id(terminal.fee_policy);
    for identity in [
        terminal_receipt_id,
        fee_record_account_id,
        settlement_candidate_id,
        fee_revenue_policy_id,
    ] {
        if identity.is_zero() {
            return Err(Error::ZeroIdentity);
        }
    }
    if epoch.phase != DealerEpochBindingPhaseV2::Leased
        || epoch.policy_id != lease.policy_id
        || epoch.facility_id != lease.facility_id
        || epoch.facility_position_binding_id != lease.facility_position_binding_id
        || epoch.dealer_state_account_id != lease.dealer_state_account_id
        || epoch.market_instance_v2_id != lease.market_instance_v2_id
        || epoch.epoch_id != lease.epoch_id
        || epoch.epoch_binding_account_id != lease.epoch_binding_account_id
        || epoch.active_lease_account_id != lease.lease_account_id
        || epoch.settlement_candidate_id != lease.settlement_candidate_id
        || settlement_candidate_id != lease.settlement_candidate_id
        || fee_record_account_id != lease.selected_fee_record_account_id
        || fee_revenue_policy_id != lease.fee_revenue_policy_id
        || terminal_receipt_id == lease.selected_fee_record_account_id
        || terminal_receipt_id == lease.lease_account_id
        || terminal_receipt_id == epoch.epoch_binding_account_id
    {
        return Err(Error::MismatchedBinding);
    }
    Ok(DealerFeeTerminalJoinV1 {
        terminal,
        terminal_receipt_id,
        selected_fee_record_account_id: lease.selected_fee_record_account_id,
        selected_fee_record_semantic_id: lease.selected_fee_record_semantic_id,
        settlement_candidate_id: lease.settlement_candidate_id,
        fee_revenue_policy_id: lease.fee_revenue_policy_id,
        outcome: terminal.outcome,
    })
}

const fn dealer_id(identity: clutch_fee_runtime_contract::Id) -> Id {
    Id::from_bytes(identity.0)
}
