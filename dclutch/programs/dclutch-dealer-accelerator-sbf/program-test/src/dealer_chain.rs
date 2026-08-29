//! Canonical Dealer selector-9 unsplit topology evidence.
//!
//! The semantic owner in `dclutch-operator::dealer_scenario_hot_v4` replays the
//! exact scenario transition, derives all nine Profile13 span widths, validates
//! the physical account geometry, and emits the canonical
//! `fixed(39) ++ admitted extras(8) ++ caller authorities ++ runtime suffix`
//! instruction. Its 121-lock canonical scenario is deliberately not submitted:
//! devnet's 64-lock limit is unchanged by an address lookup table. This module
//! owns only topology installation and rollback classification while the
//! durable preparation/commit split is built. It does not restate a span,
//! route bitmap, privilege, or account order.

use std::vec::Vec;

use dclutch_capability_program_contract::hot_v3::HOT_ROOT_ACCOUNT_V3;
use dclutch_direct_hot_program_test_support::chain::DirectHotInstallAccountV5;
use dclutch_operator::{
    dealer_scenario_hot_v4::{
        DealerScenarioHotMetaErrorV4, DealerScenarioHotMetaReportV4, DealerScenarioHotMetaStateV4,
        DealerScenarioSemanticStateV4, project_dealer_scenario_unsplit_topology_v4,
    },
    direct_inline_v3::ObservedAccountMetaV3,
};
use solana_account::Account;
use solana_program::{instruction::Instruction, pubkey::Pubkey};

/// One same-finalized Dealer bundle input.
///
/// `fixed_accounts`, `strategy_accounts`, and `runtime_suffix_accounts` are
/// chain observations, not free-form transaction metas. The operator derives
/// and authenticates their privileges and account order before this module
/// converts them into installable ProgramTest accounts.
#[derive(Clone, Copy)]
pub struct DealerScenarioChainInputV4<'a> {
    /// Exact common Hot39 observations in ABI order.
    pub fixed_accounts: &'a [ObservedAccountMetaV3],
    /// Eight admitted-AOT evidence observations followed by caller authorities.
    pub strategy_accounts: &'a [ObservedAccountMetaV3],
    /// Packed Profile13 suffix after the five fixed injected coordinates.
    pub runtime_suffix_accounts: &'a [ObservedAccountMetaV3],
    /// Same-observation semantic chain state.
    pub semantic: DealerScenarioSemanticStateV4<'a>,
    /// Exact selector-9 family request.
    pub family_request: &'a [u8],
    /// Accounts installed by the enclosing release-waist ProgramTest.
    pub externally_installed_keys: &'a [Pubkey],
}

/// One canonical Dealer chain topology before any durable split.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerScenarioUnsplitChainTopologyV4 {
    /// Unsplit Hot instruction retained only for topology analysis.
    pub hot_instruction: Instruction,
    /// All distinct fixed, strategy, and packed runtime account bodies.
    pub accounts: Vec<DirectHotInstallAccountV5>,
    /// Accounts installed externally by the release-waist harness.
    pub externally_installed_keys: Vec<Pubkey>,
    /// Mutable accounts whose state must roll back on any late refusal.
    pub rollback_snapshot_keys: Vec<Pubkey>,
    /// Canonical Dealer child root.
    pub root: Pubkey,
    /// Canonical Trading-owned obligation PDA.
    pub obligation: Pubkey,
    /// Operator-authenticated semantic and physical projection.
    pub report: DealerScenarioHotMetaReportV4,
    /// Total instruction account-meta entries.
    pub account_meta_count: usize,
    /// Exact distinct instruction locks, including the Trading program id.
    ///
    /// The eventual transaction payer is outside this unsigned fixture.
    pub unique_account_lock_count: usize,
}

/// Stable fixture-construction refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerScenarioChainErrorV4 {
    /// The semantic or physical operator refused the same-finalized view.
    Projection(DealerScenarioHotMetaErrorV4),
    /// One key carried two different observed account bodies.
    DuplicateAccountConflict,
    /// A declared externally installed key was absent from the bundle.
    MissingExternalAccount,
    /// A required fixed or semantic coordinate was absent.
    AccountGeometry,
}

impl From<DealerScenarioHotMetaErrorV4> for DealerScenarioChainErrorV4 {
    fn from(value: DealerScenarioHotMetaErrorV4) -> Self {
        Self::Projection(value)
    }
}

/// Project one exact unsplit selector-9 Hot topology.
///
/// This is deliberately the sole public constructor in the Dealer chain
/// fixture. Callers provide observations and semantic state; the operator
/// supplies every account meta, protected-span count, admitted page count, and
/// lock census. No signing or submission may occur.
pub fn project_dealer_scenario_unsplit_chain_topology_v4(
    input: DealerScenarioChainInputV4<'_>,
) -> Result<DealerScenarioUnsplitChainTopologyV4, DealerScenarioChainErrorV4> {
    let state = DealerScenarioHotMetaStateV4 {
        fixed_accounts: input.fixed_accounts,
        strategy_accounts: input.strategy_accounts,
        runtime_suffix_accounts: input.runtime_suffix_accounts,
    };
    let built =
        project_dealer_scenario_unsplit_topology_v4(state, input.semantic, input.family_request)?;
    let root = input
        .fixed_accounts
        .get(HOT_ROOT_ACCOUNT_V3)
        .ok_or(DealerScenarioChainErrorV4::AccountGeometry)?
        .account
        .key;
    let obligation = Pubkey::new_from_array(input.semantic.chain.obligation_address);

    let mut accounts = Vec::new();
    for observed in input
        .fixed_accounts
        .iter()
        .chain(input.strategy_accounts)
        .chain(input.runtime_suffix_accounts)
    {
        install_observation(&mut accounts, observed)?;
    }
    let rollback_snapshot_keys = built
        .instruction
        .accounts
        .iter()
        .filter(|meta| meta.is_writable)
        .map(|meta| meta.pubkey)
        .collect::<Vec<_>>();
    let externally_installed_keys = input
        .externally_installed_keys
        .iter()
        .map(|key| {
            accounts
                .iter()
                .any(|account| account.key == *key)
                .then_some(*key)
                .ok_or(DealerScenarioChainErrorV4::MissingExternalAccount)
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(DealerScenarioUnsplitChainTopologyV4 {
        hot_instruction: built.instruction,
        accounts,
        externally_installed_keys,
        rollback_snapshot_keys,
        root,
        obligation,
        report: built.report,
        account_meta_count: built.account_meta_count,
        unique_account_lock_count: built.unique_account_lock_count,
    })
}

fn install_observation(
    accounts: &mut Vec<DirectHotInstallAccountV5>,
    observed: &ObservedAccountMetaV3,
) -> Result<(), DealerScenarioChainErrorV4> {
    let candidate = DirectHotInstallAccountV5 {
        key: observed.account.key,
        account: Account {
            lamports: observed.account.lamports,
            data: observed.account.data.clone(),
            owner: observed.account.owner,
            executable: observed.account.executable,
            rent_epoch: 0,
        },
        snapshot_for_rollback: observed.is_writable,
    };
    if let Some(existing) = accounts
        .iter_mut()
        .find(|existing| existing.key == candidate.key)
    {
        if existing.account != candidate.account {
            return Err(DealerScenarioChainErrorV4::DuplicateAccountConflict);
        }
        existing.snapshot_for_rollback |= candidate.snapshot_for_rollback;
    } else {
        accounts.push(candidate);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_operator::{Finality, Observation, ObservedAccount};

    fn observed(key: u8, data: u8, writable: bool) -> ObservedAccountMetaV3 {
        ObservedAccountMetaV3 {
            account: ObservedAccount {
                observation: Observation {
                    slot: 7,
                    unix_timestamp: 8,
                    finality: Finality::Finalized,
                },
                key: Pubkey::new_from_array([key; 32]),
                owner: Pubkey::new_from_array([9; 32]),
                lamports: 10,
                executable: false,
                data: vec![data; 4],
            },
            is_signer: false,
            is_writable: writable,
        }
    }

    #[test]
    fn identical_aliases_install_once_and_keep_rollback_classification() {
        let mut accounts = Vec::new();
        install_observation(&mut accounts, &observed(1, 2, false)).expect("first");
        install_observation(&mut accounts, &observed(1, 2, true)).expect("alias");
        assert_eq!(accounts.len(), 1);
        assert!(
            accounts
                .first()
                .expect("one installed account")
                .snapshot_for_rollback
        );
    }

    #[test]
    fn same_key_with_a_substituted_body_refuses() {
        let mut accounts = Vec::new();
        install_observation(&mut accounts, &observed(1, 2, false)).expect("first");
        assert_eq!(
            install_observation(&mut accounts, &observed(1, 3, false)),
            Err(DealerScenarioChainErrorV4::DuplicateAccountConflict)
        );
        assert_eq!(accounts.len(), 1);
        assert_eq!(
            accounts
                .first()
                .expect("first account remains")
                .account
                .data,
            vec![2; 4]
        );
    }
}
