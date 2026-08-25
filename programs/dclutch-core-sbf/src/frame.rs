//! Exact action-specific Core account frames.

use solana_program::{account_info::AccountInfo, pubkey::Pubkey};
use solana_sdk_ids::{system_program, sysvar};

use crate::CoreSbfError;

/// Exact Found account count.
pub const FOUND_ACCOUNT_COUNT_V1: usize = 24;

/// Exact Found accounts in canonical order.
pub(crate) struct FoundAccounts<'accounts, 'info> {
    pub payer: &'accounts AccountInfo<'info>,
    pub market: &'accounts AccountInfo<'info>,
    pub rent_credit: &'accounts AccountInfo<'info>,
    pub rent_program: &'accounts AccountInfo<'info>,
    pub realm_raw: &'accounts AccountInfo<'info>,
    pub realm_staging: &'accounts AccountInfo<'info>,
    pub instance_raw: &'accounts AccountInfo<'info>,
    pub instance_staging: &'accounts AccountInfo<'info>,
    pub terms_raw: &'accounts AccountInfo<'info>,
    pub terms_staging: &'accounts AccountInfo<'info>,
    pub domain_raw: &'accounts AccountInfo<'info>,
    pub domain_staging: &'accounts AccountInfo<'info>,
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
}

impl<'accounts, 'info> FoundAccounts<'accounts, 'info> {
    pub fn parse(
        program_id: &Pubkey,
        accounts: &'accounts [AccountInfo<'info>],
    ) -> Result<Self, CoreSbfError> {
        if accounts.len() != FOUND_ACCOUNT_COUNT_V1 {
            return Err(CoreSbfError::AccountFrame);
        }
        let [
            payer,
            market,
            rent_credit,
            rent_program,
            realm_raw,
            realm_staging,
            instance_raw,
            instance_staging,
            terms_raw,
            terms_staging,
            domain_raw,
            domain_staging,
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
            instance_raw,
            instance_staging,
            terms_raw,
            terms_staging,
            domain_raw,
            domain_staging,
            resolution_raw,
            resolution_staging,
            manifest_raw,
            manifest_staging,
            release_raw,
            release_staging,
            activation_cache,
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
            instance_raw,
            instance_staging,
            terms_raw,
            terms_staging,
            domain_raw,
            domain_staging,
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
        })
    }
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
