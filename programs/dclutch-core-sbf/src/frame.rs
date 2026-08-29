//! Exact action-specific Core account frames.

use solana_program::{account_info::AccountInfo, pubkey::Pubkey};
use solana_sdk_ids::{system_program, sysvar};

use crate::CoreSbfError;

/// Exact ordinary mutating Found V3 and readonly ProjectFound V2 account counts.
pub use dclutch_market_core_codec::{FOUND_ACCOUNT_COUNT_V3, PROJECT_FOUND_ACCOUNT_COUNT_V2};
/// Exact projected generic-Found V2 prefix account count.
pub const PROJECTED_FOUND_ACCOUNT_COUNT_V2: usize = 25;
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
    pub linked_basis_raw: &'accounts AccountInfo<'info>,
    pub linked_basis_staging: &'accounts AccountInfo<'info>,
    pub resolution_raw: &'accounts AccountInfo<'info>,
    pub resolution_staging: &'accounts AccountInfo<'info>,
    pub source_spec_raw: &'accounts AccountInfo<'info>,
    pub source_spec_staging: &'accounts AccountInfo<'info>,
    pub capacity_profile_raw: &'accounts AccountInfo<'info>,
    pub capacity_profile_staging: &'accounts AccountInfo<'info>,
    pub manipulation_floor_raw: &'accounts AccountInfo<'info>,
    pub manipulation_floor_staging: &'accounts AccountInfo<'info>,
    pub manifest_raw: &'accounts AccountInfo<'info>,
    pub manifest_staging: &'accounts AccountInfo<'info>,
    pub activation_cache: &'accounts AccountInfo<'info>,
    pub core_program: &'accounts AccountInfo<'info>,
    pub core_programdata: &'accounts AccountInfo<'info>,
    pub registry_program: &'accounts AccountInfo<'info>,
    /// Present only in the mutating Found37 frame.
    pub rent: Option<&'accounts AccountInfo<'info>>,
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
        Self::parse_with_mode(program_id, accounts, true, false)
    }

    /// Parse the ordinary Found identities for a stateless projection.
    ///
    /// Projection never receives write or signature authority over the payer
    /// or future Market. All immutable authorities remain in the identical
    /// order and are authenticated by the same Found implementation. The
    /// runtime-owned Rent sysvar is omitted from the physical projection frame.
    #[inline(never)]
    pub fn parse_project(
        program_id: &Pubkey,
        accounts: &'accounts [AccountInfo<'info>],
    ) -> Result<Self, CoreSbfError> {
        Self::parse_with_mode(program_id, accounts, false, true)
    }

    #[inline(never)]
    fn parse_with_mode(
        program_id: &Pubkey,
        accounts: &'accounts [AccountInfo<'info>],
        mutating: bool,
        rent_elided: bool,
    ) -> Result<Self, CoreSbfError> {
        let expected_accounts = if rent_elided {
            PROJECT_FOUND_ACCOUNT_COUNT_V2
        } else {
            FOUND_ACCOUNT_COUNT_V3
        };
        if accounts.len() != expected_accounts {
            return Err(CoreSbfError::AccountFrame);
        }
        let ordinary = |index: usize| {
            let physical =
                if rent_elided && index > dclutch_market_core_codec::FOUND_RENT_SYSVAR_INDEX_V3 {
                    index - 1
                } else {
                    index
                };
            accounts.get(physical).ok_or(CoreSbfError::AccountFrame)
        };
        let payer = ordinary(0)?;
        let market = ordinary(1)?;
        let rent_credit = ordinary(2)?;
        let rent_program = ordinary(3)?;
        let realm_raw = ordinary(4)?;
        let realm_staging = ordinary(5)?;
        let product_raw = ordinary(6)?;
        let product_staging = ordinary(7)?;
        let result_domain_raw = ordinary(8)?;
        let result_domain_staging = ordinary(9)?;
        let portfolio_raw = ordinary(10)?;
        let portfolio_staging = ordinary(11)?;
        let linked_basis_raw = ordinary(12)?;
        let linked_basis_staging = ordinary(13)?;
        let resolution_raw = ordinary(14)?;
        let resolution_staging = ordinary(15)?;
        let source_spec_raw = ordinary(16)?;
        let source_spec_staging = ordinary(17)?;
        let capacity_profile_raw = ordinary(18)?;
        let capacity_profile_staging = ordinary(19)?;
        let manipulation_floor_raw = ordinary(20)?;
        let manipulation_floor_staging = ordinary(21)?;
        let manifest_raw = ordinary(22)?;
        let manifest_staging = ordinary(23)?;
        let activation_cache = ordinary(24)?;
        let core_program = ordinary(25)?;
        let core_programdata = ordinary(26)?;
        let registry_program = ordinary(27)?;
        let rent = if rent_elided {
            None
        } else {
            Some(ordinary(
                dclutch_market_core_codec::FOUND_RENT_SYSVAR_INDEX_V3,
            )?)
        };
        let system = ordinary(29)?;
        let infrastructure_profile = ordinary(30)?;
        let registry_artifact_raw = ordinary(31)?;
        let registry_artifact_staging = ordinary(32)?;
        let registry_programdata = ordinary(33)?;
        let rent_artifact_raw = ordinary(34)?;
        let rent_artifact_staging = ordinary(35)?;
        let rent_programdata = ordinary(36)?;
        debug_assert_eq!(
            accounts
                .get(dclutch_market_core_codec::FOUND_CAPABILITY_MANIFEST_RAW_INDEX_V3)
                .map(|account| account.key),
            Some(manifest_raw.key),
        );
        require_distinct(accounts)?;
        if payer.is_signer != mutating
            || payer.is_writable != mutating
            || payer.executable
            || market.is_signer
            || market.is_writable != mutating
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
            || rent.is_some_and(|rent| {
                rent.key != &sysvar::rent::ID
                    || rent.is_signer
                    || rent.is_writable
                    || rent.executable
            })
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
            linked_basis_raw,
            linked_basis_staging,
            resolution_raw,
            resolution_staging,
            source_spec_raw,
            source_spec_staging,
            capacity_profile_raw,
            capacity_profile_staging,
            manipulation_floor_raw,
            manipulation_floor_staging,
            manifest_raw,
            manifest_staging,
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
            linked_basis_raw,
            linked_basis_staging,
            resolution_raw,
            resolution_staging,
            source_spec_raw,
            source_spec_staging,
            capacity_profile_raw,
            capacity_profile_staging,
            manipulation_floor_raw,
            manipulation_floor_staging,
            manifest_raw,
            manifest_staging,
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

/// Compact projected generic-Found V2 prefix.
///
/// Realm/collateral, Source identity, and the principal cap come from the
/// authenticated projected-Custody state. The complete activation cache owns
/// the exact execution-release projection. Ordinary ProjectFound retains the
/// omitted finalized records and is the sole producer of those projected facts.
pub(crate) struct ProjectedFoundAccountsV2<'accounts, 'info> {
    pub payer: &'accounts AccountInfo<'info>,
    pub market: &'accounts AccountInfo<'info>,
    pub rent_credit: &'accounts AccountInfo<'info>,
    pub rent_program: &'accounts AccountInfo<'info>,
    pub product_raw: &'accounts AccountInfo<'info>,
    pub product_staging: &'accounts AccountInfo<'info>,
    pub result_domain_raw: &'accounts AccountInfo<'info>,
    pub result_domain_staging: &'accounts AccountInfo<'info>,
    pub portfolio_raw: &'accounts AccountInfo<'info>,
    pub portfolio_staging: &'accounts AccountInfo<'info>,
    pub manifest_raw: &'accounts AccountInfo<'info>,
    pub manifest_staging: &'accounts AccountInfo<'info>,
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

impl<'accounts, 'info> ProjectedFoundAccountsV2<'accounts, 'info> {
    #[inline(never)]
    pub fn parse(
        program_id: &Pubkey,
        accounts: &'accounts [AccountInfo<'info>],
    ) -> Result<Self, CoreSbfError> {
        let [
            payer,
            market,
            rent_credit,
            rent_program,
            product_raw,
            product_staging,
            result_domain_raw,
            result_domain_staging,
            portfolio_raw,
            portfolio_staging,
            manifest_raw,
            manifest_staging,
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
        for readonly in [
            product_raw,
            product_staging,
            result_domain_raw,
            result_domain_staging,
            portfolio_raw,
            portfolio_staging,
            manifest_raw,
            manifest_staging,
            activation_cache,
            registry_artifact_raw,
            registry_artifact_staging,
            rent_artifact_raw,
            rent_artifact_staging,
        ] {
            if readonly.is_signer || readonly.is_writable || readonly.executable {
                return Err(CoreSbfError::AccountFrame);
            }
        }
        Ok(Self {
            payer,
            market,
            rent_credit,
            rent_program,
            product_raw,
            product_staging,
            result_domain_raw,
            result_domain_staging,
            portfolio_raw,
            portfolio_staging,
            manifest_raw,
            manifest_staging,
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
