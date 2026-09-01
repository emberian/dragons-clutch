//! Chain-authenticated planning for the post-founding Resolution readiness walk.
//!
//! This module does not own Market or funding semantics. It acquires one
//! finalized account snapshot, supplies it to the canonical Resolution Core V3
//! builders, and routes only combinations those builders authenticate. The
//! reports remain the semantic owners of instruction and economic facts.

use dclutch_resolution_core_v3_operator::{Finality, Observation, ObservedAccount};
pub(crate) use dclutch_source_readiness_operator::{
    FundingReadinessCoordinatesV1, FundingReadinessInstructionPlanV1, FundingReadinessPlanV1,
    FundingReadinessPrepayV1, FundingReadinessRecordCoordinatesV1,
};
use dclutch_source_readiness_operator::{
    FundingReadinessFrameV1, funding_readiness_observation_addresses_v1, plan_funding_readiness_v1,
};
use solana_address_lookup_table_interface::{
    program as lookup_table_program, state::AddressLookupTable,
};
use solana_sdk::pubkey::Pubkey;
use solana_sdk_ids::system_program;

use crate::{
    Error, Result,
    model::SuccessorPlan,
    plan::pubkey,
    rpc::{Rpc, RpcAccount},
};

/// One semantic readiness plan and address-table accounts observed in the
/// same finalized RPC response. Routing remains non-authoritative, but the v0
/// compiler may not relabel stale table bytes with the semantic observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FundingReadinessRoutedPlanV1 {
    pub(crate) plan: FundingReadinessPlanV1,
    pub(crate) routing_tables: Vec<ObservedAccount>,
}

/// Acquire and route one bounded, same-finalized readiness snapshot.
pub(crate) fn plan_funding_readiness_from_rpc_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    coordinates: FundingReadinessCoordinatesV1,
    minimum_slot: u64,
) -> Result<FundingReadinessPlanV1> {
    Ok(
        plan_funding_readiness_with_routing_from_rpc_v1(rpc, plan, coordinates, minimum_slot, &[])?
            .plan,
    )
}

/// Return every address the shared founding ALT must contain before Open.
pub(crate) fn funding_readiness_routing_addresses_v1(
    plan: &SuccessorPlan,
    coordinates: FundingReadinessCoordinatesV1,
) -> Result<Vec<Pubkey>> {
    let frame = funding_readiness_frame_v1(plan, coordinates)?;
    funding_readiness_observation_addresses_v1(&frame).map_err(|error| refusal(error.message()))
}

/// Acquire semantic accounts and caller-selected routing tables in one exact
/// finalized snapshot, then construct the canonical next readiness action.
pub(crate) fn plan_funding_readiness_with_routing_from_rpc_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    coordinates: FundingReadinessCoordinatesV1,
    minimum_slot: u64,
    routing_table_keys: &[Pubkey],
) -> Result<FundingReadinessRoutedPlanV1> {
    let frame = funding_readiness_frame_v1(plan, coordinates)?;
    let semantic_addresses = funding_readiness_observation_addresses_v1(&frame)
        .map_err(|error| refusal(error.message()))?;
    let mut addresses = semantic_addresses.clone();
    for key in routing_table_keys {
        if addresses.contains(key) {
            return Err(refusal(
                "funding-readiness routing table aliased a semantic account or another table",
            ));
        }
        addresses.push(*key);
    }
    let (slot, accounts) = rpc.finalized_accounts(&addresses, minimum_slot)?;
    let observation = Observation {
        slot,
        unix_timestamp: rpc.block_time(slot)?,
        finality: Finality::Finalized,
    };
    let mut accounts = accounts;
    let routing_accounts = accounts.split_off(semantic_addresses.len());
    let observed = semantic_addresses
        .iter()
        .copied()
        .zip(accounts)
        .map(|(key, account)| {
            let account = account.unwrap_or_else(vacant_rpc_account_v1);
            ObservedAccount {
                observation,
                key,
                owner: account.owner,
                lamports: account.lamports,
                executable: account.executable,
                data: account.data,
            }
        })
        .collect::<Vec<_>>();
    let routing_tables = routing_table_keys
        .iter()
        .copied()
        .zip(routing_accounts)
        .map(|(key, account)| {
            let account = account.ok_or_else(|| {
                refusal("funding-readiness finalized snapshot omitted a routing table")
            })?;
            authenticate_routing_table_v1(observation, key, account)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(FundingReadinessRoutedPlanV1 {
        plan: plan_funding_readiness_v1(&frame, &observed)
            .map_err(|error| refusal(error.message()))?,
        routing_tables,
    })
}

fn authenticate_routing_table_v1(
    observation: Observation,
    key: Pubkey,
    account: RpcAccount,
) -> Result<ObservedAccount> {
    let table = AddressLookupTable::deserialize(&account.data)
        .map_err(|_| refusal("funding-readiness routing table bytes were invalid"))?;
    if account.owner != lookup_table_program::ID
        || account.executable
        || table.meta.authority.is_some()
        || table.meta.deactivation_slot != u64::MAX
        || table.meta.last_extended_slot >= observation.slot
        || table.addresses.is_empty()
    {
        return Err(refusal(
            "funding-readiness routing table was not exact, frozen, active, and activated",
        ));
    }
    Ok(ObservedAccount {
        observation,
        key,
        owner: account.owner,
        lamports: account.lamports,
        executable: account.executable,
        data: account.data,
    })
}

fn funding_readiness_frame_v1(
    plan: &SuccessorPlan,
    coordinates: FundingReadinessCoordinatesV1,
) -> Result<FundingReadinessFrameV1> {
    let frame = FundingReadinessFrameV1 {
        coordinates,
        activation_cache: pubkey(&plan.activation)?,
        registry_program: pubkey(&plan.registry.program_id)?,
        core_program: pubkey(&plan.core.program_id)?,
        core_programdata: pubkey(&plan.core.programdata_id)?,
        resolution_program: pubkey(&plan.resolution.program_id)?,
        resolution_programdata: pubkey(&plan.resolution.programdata_id)?,
    };
    funding_readiness_observation_addresses_v1(&frame).map_err(|error| refusal(error.message()))?;
    Ok(frame)
}

fn vacant_rpc_account_v1() -> RpcAccount {
    RpcAccount {
        lamports: 0,
        owner: system_program::ID,
        executable: false,
        rent_epoch: 0,
        data: Vec::new(),
    }
}

fn refusal(message: impl Into<String>) -> Error {
    Error::new(message)
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use solana_address_lookup_table_interface::state::LookupTableMeta;

    use super::*;

    fn key() -> Pubkey {
        Pubkey::new_unique()
    }

    fn routing_account(
        authority: Option<Pubkey>,
        deactivation_slot: u64,
        last_extended_slot: u64,
    ) -> RpcAccount {
        let table = AddressLookupTable {
            meta: LookupTableMeta {
                deactivation_slot,
                last_extended_slot,
                last_extended_slot_start_index: 0,
                authority,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(vec![key(), key()]),
        };
        RpcAccount {
            lamports: 1,
            owner: lookup_table_program::ID,
            executable: false,
            rent_epoch: 0,
            data: table.serialize_for_tests().expect("table bytes"),
        }
    }

    #[test]
    fn routing_table_requires_frozen_active_activated_exact_account() {
        let observation = Observation {
            slot: 9,
            unix_timestamp: 1,
            finality: Finality::Finalized,
        };
        assert!(
            authenticate_routing_table_v1(observation, key(), routing_account(None, u64::MAX, 8),)
                .is_ok()
        );
        for account in [
            routing_account(Some(key()), u64::MAX, 8),
            routing_account(None, 7, 8),
            routing_account(None, u64::MAX, 9),
            RpcAccount {
                owner: key(),
                ..routing_account(None, u64::MAX, 8)
            },
        ] {
            assert!(authenticate_routing_table_v1(observation, key(), account).is_err());
        }
    }
}
