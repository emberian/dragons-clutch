//! Chain-authenticated planning for the post-founding Resolution readiness walk.
//!
//! This module does not own Market or funding semantics. It acquires one
//! finalized account snapshot, supplies it to the canonical Resolution Core V3
//! builders, and routes only combinations those builders authenticate. The
//! reports remain the semantic owners of instruction and economic facts.

use dclutch_resolution_core_v3_operator::{Finality, Observation, ObservedAccount};
pub(crate) use dclutch_operator::source_readiness::{
    FundingReadinessCoordinatesV1, FundingReadinessInstructionPlanV1, FundingReadinessPlanV1,
    FundingReadinessPrepayV1, FundingReadinessRecordCoordinatesV1,
};
use dclutch_operator::source_readiness::{
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
    rpc::{NonAuthoritativeBlockTimeV1, Rpc, RpcAccount},
};

/// The clock an observation carries when the endpoint would not serve one.
///
/// This site has NO clock consumer: `plan_funding_readiness_v1` reads an
/// observation's slot and its accounts and never its clock, and
/// `the_funding_readiness_planner_has_no_observation_clock_consumer_v1` is the
/// positive control. So this is a declared absence at a site with no reader,
/// not a substituted time at a site with one. The absence itself is carried
/// out in [`FundingReadinessRoutedPlanV1`] and restated in the run's report.
const UNREAD_OBSERVATION_CLOCK_V1: i64 = 0;

/// One semantic readiness plan and address-table accounts observed in the
/// same finalized RPC response. Routing remains non-authoritative, but the v0
/// compiler may not relabel stale table bytes with the semantic observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FundingReadinessRoutedPlanV1 {
    pub(crate) plan: FundingReadinessPlanV1,
    pub(crate) routing_tables: Vec<ObservedAccount>,
    /// What the endpoint said about this observation's wall clock. Never a
    /// refusal: no part of this plan reads it.
    pub(crate) observation_block_time: NonAuthoritativeBlockTimeV1,
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
    let observation_block_time = rpc.non_authoritative_block_time(slot);
    let observation = Observation {
        slot,
        unix_timestamp: observation_block_time
            .unix_timestamp
            .unwrap_or(UNREAD_OBSERVATION_CLOCK_V1),
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
        observation_block_time,
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

    /// The positive control for [`UNREAD_OBSERVATION_CLOCK_V1`].
    ///
    /// `plan_funding_readiness_with_routing_from_rpc_v1` may degrade a refused
    /// `getBlockTime` to a declared absence only for as long as nothing in the
    /// PLAN reads the clock it stamps onto the observation. So this reads the
    /// operator's own planning sources and refuses if the word appears in them
    /// at all -- the day someone gives the plan a clock, this goes red before
    /// a run can carry a zero into it.
    ///
    /// `wire.rs` is excluded and named rather than swept in, because it is a
    /// serializer: it carries an observation's clock across a JSON boundary
    /// for a caller that supplied one, and decides nothing from it. The
    /// exclusion is asserted to be non-vacuous below, so it cannot quietly
    /// become a hole if that file is renamed away.
    #[test]
    fn the_funding_readiness_planner_has_no_observation_clock_consumer_v1() {
        let root = crate::model::repository_root_v1()
            .join("crates/dclutch-source-readiness-operator/src");
        let mut sources = Vec::new();
        let mut pending = vec![root.clone()];
        while let Some(directory) = pending.pop() {
            for entry in std::fs::read_dir(&directory)
                .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
            {
                let path = entry.expect("readiness operator directory entry").path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|value| value == "rs") {
                    sources.push(path);
                }
            }
        }
        assert!(
            sources
                .iter()
                .any(|path| path.file_name().is_some_and(|name| name == "wire.rs")),
            "the excluded serializer is gone, so this control is measuring a different crate"
        );
        assert!(
            sources
                .iter()
                .any(|path| path.file_name().is_some_and(|name| name == "lib.rs")),
            "the readiness planner source was not found under {}",
            root.display()
        );
        let carriers = sources
            .iter()
            .filter(|path| path.file_name().is_none_or(|name| name != "wire.rs"))
            .filter(|path| {
                std::fs::read_to_string(path)
                    .expect("readiness operator source")
                    .contains("unix_timestamp")
            })
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        assert!(
            carriers.is_empty(),
            "the funding-readiness plan now reads an observation clock ({}), so a refused \
             getBlockTime may no longer degrade to UNREAD_OBSERVATION_CLOCK_V1",
            carriers.join(", ")
        );
    }
}
