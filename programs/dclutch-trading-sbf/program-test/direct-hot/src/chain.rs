//! Dependency-narrow Direct Hot chain-account installation.
//!
//! The full host operator intentionally is not a dependency of this ProgramTest
//! support crate: its transaction-routing dependency versions differ from
//! ProgramTest's. This module installs only Direct-owned fixture accounts and
//! preserves the release waist already installed by the Registry campaign.

use solana_account::Account;
use solana_program::{pubkey::Pubkey, rent::Rent};
use solana_program_test::ProgramTest;

/// One exact account installed by the shared Direct chain fixture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectHotInstallAccountV5 {
    /// Exact account identity.
    pub key: Pubkey,
    /// Exact initial account state.
    pub account: Account,
    /// Whether late child refusal must preserve this account byte-for-byte.
    pub snapshot_for_rollback: bool,
}

/// Result of installing the Direct-owned portion of one shared chain fixture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledDirectHotChainV5 {
    /// Direct-owned accounts actually added to ProgramTest.
    pub installed_keys: Vec<Pubkey>,
    /// Exact material state keys to snapshot before the outer Registry call.
    pub rollback_snapshot_keys: Vec<Pubkey>,
}

/// Stable refusal from Direct fixture installation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectHotChainErrorV5 {
    /// A key was zero or repeated in the complete account declaration.
    InvalidIdentity,
    /// A data account was below the supplied canonical Rent minimum.
    UnderfundedAccount,
    /// A declared external release account was absent from the complete fixture.
    UnknownExternalAccount,
}

/// Install exact Direct-owned chain accounts while preserving externally
/// installed programs, ProgramData, activation cache, and release-waist state.
///
/// `externally_installed` must name accounts in `accounts`; this makes the
/// ownership split auditable and prevents accidentally omitting a fixture fact.
/// The caller remains the sole owner of external release setup and may append
/// admission/cache keys to the returned rollback snapshot set.
pub fn install_direct_hot_chain_accounts_v5(
    test: &mut ProgramTest,
    rent: &Rent,
    accounts: &[DirectHotInstallAccountV5],
    externally_installed: &[Pubkey],
) -> Result<InstalledDirectHotChainV5, DirectHotChainErrorV5> {
    validate_unique_nonzero(accounts, externally_installed)?;
    let mut installed_keys = Vec::with_capacity(accounts.len());
    let mut rollback_snapshot_keys = Vec::new();
    for candidate in accounts {
        if !candidate.account.data.is_empty()
            && candidate.account.lamports < rent.minimum_balance(candidate.account.data.len())
        {
            return Err(DirectHotChainErrorV5::UnderfundedAccount);
        }
        if candidate.snapshot_for_rollback {
            rollback_snapshot_keys.push(candidate.key);
        }
        if !externally_installed.contains(&candidate.key) {
            test.add_account(candidate.key, candidate.account.clone());
            installed_keys.push(candidate.key);
        }
    }
    Ok(InstalledDirectHotChainV5 {
        installed_keys,
        rollback_snapshot_keys,
    })
}

fn validate_unique_nonzero(
    accounts: &[DirectHotInstallAccountV5],
    externally_installed: &[Pubkey],
) -> Result<(), DirectHotChainErrorV5> {
    let mut keys = Vec::with_capacity(accounts.len());
    for candidate in accounts {
        if candidate.key == Pubkey::default() || keys.contains(&candidate.key) {
            return Err(DirectHotChainErrorV5::InvalidIdentity);
        }
        keys.push(candidate.key);
    }
    for external in externally_installed {
        if *external == Pubkey::default() || !keys.contains(external) {
            return Err(DirectHotChainErrorV5::UnknownExternalAccount);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(rent: &Rent, owner: Pubkey, byte: u8) -> Account {
        let data = vec![byte; 8];
        Account {
            lamports: rent.minimum_balance(data.len()),
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        }
    }

    #[test]
    fn installer_preserves_release_accounts_and_returns_rollback_set() {
        let external = Pubkey::new_from_array([21; 32]);
        let local = Pubkey::new_from_array([22; 32]);
        let owner = Pubkey::new_from_array([23; 32]);
        let rent = Rent::default();
        let mut test = ProgramTest::default();
        let installed = install_direct_hot_chain_accounts_v5(
            &mut test,
            &rent,
            &[
                DirectHotInstallAccountV5 {
                    key: external,
                    account: account(&rent, owner, 1),
                    snapshot_for_rollback: false,
                },
                DirectHotInstallAccountV5 {
                    key: local,
                    account: account(&rent, owner, 2),
                    snapshot_for_rollback: true,
                },
            ],
            &[external],
        )
        .expect("install");
        assert_eq!(installed.installed_keys, vec![local]);
        assert_eq!(installed.rollback_snapshot_keys, vec![local]);
    }

    #[test]
    fn duplicates_unknown_externals_and_underfunding_refuse() {
        let key = Pubkey::new_from_array([31; 32]);
        let owner = Pubkey::new_from_array([32; 32]);
        let rent = Rent::default();
        let candidate = DirectHotInstallAccountV5 {
            key,
            account: account(&rent, owner, 3),
            snapshot_for_rollback: true,
        };
        assert_eq!(
            install_direct_hot_chain_accounts_v5(
                &mut ProgramTest::default(),
                &rent,
                &[candidate.clone(), candidate.clone()],
                &[],
            ),
            Err(DirectHotChainErrorV5::InvalidIdentity)
        );
        assert_eq!(
            install_direct_hot_chain_accounts_v5(
                &mut ProgramTest::default(),
                &rent,
                std::slice::from_ref(&candidate),
                &[Pubkey::new_from_array([33; 32])],
            ),
            Err(DirectHotChainErrorV5::UnknownExternalAccount)
        );
        let mut underfunded = candidate;
        underfunded.account.lamports = 0;
        assert_eq!(
            install_direct_hot_chain_accounts_v5(
                &mut ProgramTest::default(),
                &rent,
                &[underfunded],
                &[],
            ),
            Err(DirectHotChainErrorV5::UnderfundedAccount)
        );
    }
}
