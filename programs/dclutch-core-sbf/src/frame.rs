//! Exact action-specific Core account frames.

use solana_program::{account_info::AccountInfo, pubkey::Pubkey};
use solana_sdk_ids::{system_program, sysvar};

use crate::CoreSbfError;

/// Exact Found account count with append-only infrastructure authority.
pub const FOUND_ACCOUNT_COUNT_V2: usize = 31;
/// Exact account count for one-time infrastructure-profile initialization.
pub const INITIALIZE_INFRASTRUCTURE_ACCOUNT_COUNT_V1: usize = 14;

/// Exact Found accounts in canonical order.
pub(crate) struct FoundAccounts<'accounts, 'info> {
    pub payer: &'accounts AccountInfo<'info>,
    pub market: &'accounts AccountInfo<'info>,
    pub rent_credit: &'accounts AccountInfo<'info>,
    pub rent_program: &'accounts AccountInfo<'info>,
    pub realm_raw: &'accounts AccountInfo<'info>,
    pub realm_staging: &'accounts AccountInfo<'info>,
    pub product_raw: &'accounts AccountInfo<'info>,
    pub product_staging: &'accounts AccountInfo<'info>,
    pub result_domain_raw: &'accounts AccountInfo<'info>,
    pub result_domain_staging: &'accounts AccountInfo<'info>,
    pub portfolio_raw: &'accounts AccountInfo<'info>,
    pub portfolio_staging: &'accounts AccountInfo<'info>,
    pub resolution_raw: &'accounts AccountInfo<'info>,
    pub resolution_staging: &'accounts AccountInfo<'info>,
    pub manifest_raw: &'accounts AccountInfo<'info>,
    pub manifest_staging: &'accounts AccountInfo<'info>,
    pub release_raw: &'accounts AccountInfo<'info>,
    pub release_staging: &'accounts AccountInfo<'info>,
    pub activation_cache: &'accounts AccountInfo<'info>,
    pub core_program: &'accounts AccountInfo<'info>,
    pub core_programdata: &'accounts AccountInfo<'info>,
    pub registry_program: &'accounts AccountInfo<'info>,
    pub rent: &'accounts AccountInfo<'info>,
    pub system: &'accounts AccountInfo<'info>,
    pub infrastructure_profile: &'accounts AccountInfo<'info>,
    pub registry_artifact_raw: &'accounts AccountInfo<'info>,
    pub registry_artifact_staging: &'accounts AccountInfo<'info>,
    pub registry_programdata: &'accounts AccountInfo<'info>,
    pub rent_artifact_raw: &'accounts AccountInfo<'info>,
    pub rent_artifact_staging: &'accounts AccountInfo<'info>,
    pub rent_programdata: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> FoundAccounts<'accounts, 'info> {
    #[inline(never)]
    pub fn parse(
        program_id: &Pubkey,
        accounts: &'accounts [AccountInfo<'info>],
    ) -> Result<Self, CoreSbfError> {
        if accounts.len() != FOUND_ACCOUNT_COUNT_V2 {
            return Err(CoreSbfError::AccountFrame);
        }
        let [
            payer,
            market,
            rent_credit,
            rent_program,
            realm_raw,
            realm_staging,
            product_raw,
            product_staging,
            result_domain_raw,
            result_domain_staging,
            portfolio_raw,
            portfolio_staging,
            resolution_raw,
            resolution_staging,
            manifest_raw,
            manifest_staging,
            release_raw,
            release_staging,
            activation_cache,
            core_program,
            core_programdata,
            registry_program,
            rent,
            system,
            infrastructure_profile,
            registry_artifact_raw,
            registry_artifact_staging,
            registry_programdata,
            rent_artifact_raw,
            rent_artifact_staging,
            rent_programdata,
        ] = accounts
        else {
            return Err(CoreSbfError::AccountFrame);
        };
        require_distinct(accounts)?;
        if !payer.is_signer
            || !payer.is_writable
            || payer.executable
            || market.is_signer
            || !market.is_writable
            || market.executable
            || rent_credit.is_signer
            || rent_credit.is_writable
            || rent_credit.executable
            || rent_program.is_signer
            || rent_program.is_writable
            || !rent_program.executable
            || core_program.key != program_id
            || core_program.is_signer
            || core_program.is_writable
            || !core_program.executable
            || core_programdata.is_signer
            || core_programdata.is_writable
            || core_programdata.executable
            || registry_program.is_signer
            || registry_program.is_writable
            || !registry_program.executable
            || infrastructure_profile.is_signer
            || infrastructure_profile.is_writable
            || infrastructure_profile.executable
            || registry_programdata.is_signer
            || registry_programdata.is_writable
            || registry_programdata.executable
            || rent_programdata.is_signer
            || rent_programdata.is_writable
            || rent_programdata.executable
            || rent.key != &sysvar::rent::ID
            || rent.is_signer
            || rent.is_writable
            || rent.executable
            || system.key != &system_program::ID
            || system.is_signer
            || system.is_writable
            || !system.executable
        {
            return Err(CoreSbfError::AccountFrame);
        }
        for account in [
            realm_raw,
            realm_staging,
            product_raw,
            product_staging,
            result_domain_raw,
            result_domain_staging,
            portfolio_raw,
            portfolio_staging,
            resolution_raw,
            resolution_staging,
            manifest_raw,
            manifest_staging,
            release_raw,
            release_staging,
            activation_cache,
            registry_artifact_raw,
            registry_artifact_staging,
            rent_artifact_raw,
            rent_artifact_staging,
        ] {
            if account.is_signer || account.is_writable || account.executable {
                return Err(CoreSbfError::AccountFrame);
            }
        }
        Ok(Self {
            payer,
            market,
            rent_credit,
            rent_program,
            realm_raw,
            realm_staging,
            product_raw,
            product_staging,
            result_domain_raw,
            result_domain_staging,
            portfolio_raw,
            portfolio_staging,
            resolution_raw,
            resolution_staging,
            manifest_raw,
            manifest_staging,
            release_raw,
            release_staging,
            activation_cache,
            core_program,
            core_programdata,
            registry_program,
            rent,
            system,
            infrastructure_profile,
            registry_artifact_raw,
            registry_artifact_staging,
            registry_programdata,
            rent_artifact_raw,
            rent_artifact_staging,
            rent_programdata,
        })
    }
}

/// Exact accounts for one-time infrastructure-profile initialization.
pub(crate) struct InitializeInfrastructureAccounts<'accounts, 'info> {
    pub payer: &'accounts AccountInfo<'info>,
    pub profile: &'accounts AccountInfo<'info>,
    pub core_programdata: &'accounts AccountInfo<'info>,
    pub upgrade_authority: &'accounts AccountInfo<'info>,
    pub registry_artifact_raw: &'accounts AccountInfo<'info>,
    pub registry_artifact_staging: &'accounts AccountInfo<'info>,
    pub registry_program: &'accounts AccountInfo<'info>,
    pub registry_programdata: &'accounts AccountInfo<'info>,
    pub rent_artifact_raw: &'accounts AccountInfo<'info>,
    pub rent_artifact_staging: &'accounts AccountInfo<'info>,
    pub rent_program: &'accounts AccountInfo<'info>,
    pub rent_programdata: &'accounts AccountInfo<'info>,
    pub rent: &'accounts AccountInfo<'info>,
    pub system: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> InitializeInfrastructureAccounts<'accounts, 'info> {
    #[inline(never)]
    pub fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, CoreSbfError> {
        if accounts.len() != INITIALIZE_INFRASTRUCTURE_ACCOUNT_COUNT_V1 {
            return Err(CoreSbfError::AccountFrame);
        }
        let [
            payer,
            profile,
            core_programdata,
            upgrade_authority,
            registry_artifact_raw,
            registry_artifact_staging,
            registry_program,
            registry_programdata,
            rent_artifact_raw,
            rent_artifact_staging,
            rent_program,
            rent_programdata,
            rent,
            system,
        ] = accounts
        else {
            return Err(CoreSbfError::AccountFrame);
        };
        if !payer.is_signer
            || !payer.is_writable
            || payer.executable
            || profile.is_signer
            || !profile.is_writable
            || profile.executable
            || core_programdata.is_signer
            || core_programdata.is_writable
            || core_programdata.executable
            || !upgrade_authority.is_signer
            || upgrade_authority.executable
            || registry_program.is_signer
            || registry_program.is_writable
            || !registry_program.executable
            || registry_programdata.is_signer
            || registry_programdata.is_writable
            || registry_programdata.executable
            || rent_program.is_signer
            || rent_program.is_writable
            || !rent_program.executable
            || rent_programdata.is_signer
            || rent_programdata.is_writable
            || rent_programdata.executable
            || rent.key != &sysvar::rent::ID
            || rent.is_signer
            || rent.is_writable
            || rent.executable
            || system.key != &system_program::ID
            || system.is_signer
            || system.is_writable
            || !system.executable
        {
            return Err(CoreSbfError::AccountFrame);
        }
        for account in [
            registry_artifact_raw,
            registry_artifact_staging,
            rent_artifact_raw,
            rent_artifact_staging,
        ] {
            if account.is_signer || account.is_writable || account.executable {
                return Err(CoreSbfError::AccountFrame);
            }
        }
        require_distinct_except_payer_authority(accounts)?;
        Ok(Self {
            payer,
            profile,
            core_programdata,
            upgrade_authority,
            registry_artifact_raw,
            registry_artifact_staging,
            registry_program,
            registry_programdata,
            rent_artifact_raw,
            rent_artifact_staging,
            rent_program,
            rent_programdata,
            rent,
            system,
        })
    }
}

fn require_distinct_except_payer_authority(
    accounts: &[AccountInfo<'_>],
) -> Result<(), CoreSbfError> {
    for (left_index, left) in accounts.iter().enumerate() {
        for (right_index, right) in accounts
            .iter()
            .enumerate()
            .skip(left_index.saturating_add(1))
        {
            if left.key == right.key && !matches!((left_index, right_index), (0, 3)) {
                return Err(CoreSbfError::AccountFrame);
            }
        }
    }
    Ok(())
}

pub(crate) fn require_distinct(accounts: &[AccountInfo<'_>]) -> Result<(), CoreSbfError> {
    for (left_index, left) in accounts.iter().enumerate() {
        for right in accounts.iter().skip(left_index.saturating_add(1)) {
            if left.key == right.key {
                return Err(CoreSbfError::AccountFrame);
            }
        }
    }
    Ok(())
}
