//! Exact action-specific Core account frames.

use solana_program::{account_info::AccountInfo, pubkey::Pubkey};
use solana_sdk_ids::{system_program, sysvar};

use crate::CoreSbfError;

/// Exact ordinary mutating Found V3 and readonly ProjectFound V2 account counts.
pub use dclutch_market_core_codec::{
    FOUND_ACCOUNT_COUNT_V3, FOUND_PRICE_GATE_ACCOUNT_COUNT_V3, PROJECT_FOUND_ACCOUNT_COUNT_V2,
    PROJECT_FOUND_PRICE_GATE_ACCOUNT_COUNT_V2,
};
/// Exact projected generic-Found V2 prefix account count.
pub const PROJECTED_FOUND_ACCOUNT_COUNT_V2: usize = 24;
/// Exact account count for one-time infrastructure-profile initialization.
pub const INITIALIZE_INFRASTRUCTURE_ACCOUNT_COUNT_V1: usize = 15;
/// Exact account count for the one-time infrastructure succession ceremony.
pub const INITIALIZE_INFRASTRUCTURE_ACCOUNT_COUNT_V2: usize = 21;

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
    /// The `DCLTPGT1` no-arbitrage certificate pair, present exactly when
    /// the caller offered the extended frame.
    ///
    /// `None` is not "no certificate needed" -- it is "none offered". Whether
    /// one was *required* is a property of the basis record, decided in
    /// `authenticate_references` once the basis has been authenticated.
    pub price_gate: Option<(&'accounts AccountInfo<'info>, &'accounts AccountInfo<'info>)>,
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
        // **Two admissible widths, not one**, exactly as the Rent sysvar
        // already does. The trailing pair carries a `DCLTPGT1` no-arbitrage
        // certificate, which a Market needs precisely when its basis declares
        // degree >= 2. Every categorical and graded founding has no such record
        // and would otherwise have to invent a placeholder for it.
        //
        // Nothing before the pair moves, so a caller that forwards the
        // canonical frame is unaffected -- and correspondingly cannot found a
        // curved basis, which `authenticate_references` refuses by name rather
        // than by a length mismatch.
        let (bare, extended) = if rent_elided {
            (
                PROJECT_FOUND_ACCOUNT_COUNT_V2,
                PROJECT_FOUND_PRICE_GATE_ACCOUNT_COUNT_V2,
            )
        } else {
            (FOUND_ACCOUNT_COUNT_V3, FOUND_PRICE_GATE_ACCOUNT_COUNT_V3)
        };
        let price_gate_offered = accounts.len() == extended;
        if accounts.len() != bare && !price_gate_offered {
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
        let price_gate = if price_gate_offered {
            Some((
                ordinary(dclutch_market_core_codec::FOUND_PRICE_GATE_RAW_INDEX_V3)?,
                ordinary(dclutch_market_core_codec::FOUND_PRICE_GATE_STAGING_INDEX_V3)?,
            ))
        } else {
            None
        };
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
        // The certificate pair is authenticated in full by
        // `authenticate_record` -- ownership, PDA, digest and rent exemption --
        // but its account-shape discipline belongs here with every other
        // readonly record in the frame.
        if let Some((raw, staging)) = price_gate {
            for account in [raw, staging] {
                if account.is_signer || account.is_writable || account.executable {
                    return Err(CoreSbfError::AccountFrame);
                }
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
            price_gate,
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
    /// The V2 PDA this genesis initialization also writes.
    ///
    /// A cohort that succeeds nothing commits BOTH profiles in one instruction:
    /// the sealed V1 historical record, and the genesis V2 every consumer
    /// actually reads. One instruction so a cohort can never stand half
    /// initialized, with a V1 nothing reads and no V2 to found against.
    pub genesis_profile: &'accounts AccountInfo<'info>,
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
            genesis_profile,
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
            || genesis_profile.is_signer
            || !genesis_profile.is_writable
            || genesis_profile.executable
            || genesis_profile.key == profile.key
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
            genesis_profile,
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

/// Exact accounts for the one-time infrastructure succession ceremony.
///
/// The V1 initialization frame plus the predecessor evidence the ruling's
/// conjuncts read: the V1 profile itself, the predecessor artifact records
/// for each MOVED binding, and one consent slot per binding. An UNMOVED
/// binding presents the System program in its predecessor-record and consent
/// slots, exactly as `DeclareSuccessor` does for an unmoved role — nothing is
/// being consented to, so nothing may stand there that could look like
/// consent.
pub(crate) struct InitializeInfrastructureV2Accounts<'accounts, 'info> {
    pub payer: &'accounts AccountInfo<'info>,
    /// The vacant V2 profile PDA (conjunct 6 owns its vacancy).
    pub profile: &'accounts AccountInfo<'info>,
    /// The written V1 profile PDA (conjunct 2 owns its presence).
    pub predecessor_profile: &'accounts AccountInfo<'info>,
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
    pub predecessor_registry_artifact_raw: &'accounts AccountInfo<'info>,
    pub predecessor_registry_artifact_staging: &'accounts AccountInfo<'info>,
    pub registry_consent_authority: &'accounts AccountInfo<'info>,
    pub predecessor_rent_artifact_raw: &'accounts AccountInfo<'info>,
    pub predecessor_rent_artifact_staging: &'accounts AccountInfo<'info>,
    pub rent_consent_authority: &'accounts AccountInfo<'info>,
    pub rent: &'accounts AccountInfo<'info>,
    pub system: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> InitializeInfrastructureV2Accounts<'accounts, 'info> {
    #[inline(never)]
    pub fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, CoreSbfError> {
        if accounts.len() != INITIALIZE_INFRASTRUCTURE_ACCOUNT_COUNT_V2 {
            return Err(CoreSbfError::AccountFrame);
        }
        let [
            payer,
            profile,
            predecessor_profile,
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
            predecessor_registry_artifact_raw,
            predecessor_registry_artifact_staging,
            registry_consent_authority,
            predecessor_rent_artifact_raw,
            predecessor_rent_artifact_staging,
            rent_consent_authority,
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
            || predecessor_profile.is_signer
            || predecessor_profile.is_writable
            || predecessor_profile.executable
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
        // The unmoved-arm slots. An UNMOVED binding stands the System program
        // here, and every runtime presents that account as executable — the
        // same mutual-satisfiability trap `DeclareSuccessor` hit (d6e43b11),
        // exempted the same way. The exemption decides nothing: whether the
        // System program may stand in a given slot is the succession arm's
        // call (conjuncts 4 and 5), where it is admitted only for a binding
        // that did NOT move. Every other executable account is refused here.
        for account in [
            predecessor_registry_artifact_raw,
            predecessor_registry_artifact_staging,
            predecessor_rent_artifact_raw,
            predecessor_rent_artifact_staging,
        ] {
            if account.is_signer
                || account.is_writable
                || (account.executable && account.key != &system_program::ID)
            {
                return Err(CoreSbfError::AccountFrame);
            }
        }
        // Whether a consent slot must sign is a function of whether its
        // binding moved, which is not known until the records are decoded.
        // The succession arm owns the signer bit; the frame refuses only what
        // could never be a consent: a writable slot, or an executable one
        // that is not the System program standing for "no consent demanded".
        for slot in [registry_consent_authority, rent_consent_authority] {
            if slot.is_writable || (slot.executable && slot.key != &system_program::ID) {
                return Err(CoreSbfError::AccountFrame);
            }
        }
        require_distinct_for_succession(accounts)?;
        Ok(Self {
            payer,
            profile,
            predecessor_profile,
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
            predecessor_registry_artifact_raw,
            predecessor_registry_artifact_staging,
            registry_consent_authority,
            predecessor_rent_artifact_raw,
            predecessor_rent_artifact_staging,
            rent_consent_authority,
            rent,
            system,
        })
    }
}

/// Distinctness for the succession frame, with exactly two named exemptions.
///
/// 1. The natural-person slots — payer (0), Core's upgrade authority (4), and
///    the two consent authorities (15, 18) — may share keys freely: on devnet
///    they are one key, and conjunct 5 constrains what each must SIGN, not
///    that the humans be distinct people.
/// 2. Any two slots that both hold the System program may alias: an unmoved
///    binding stands `system_program::ID` in up to three slots beside the
///    frame's own System account (20).
///
/// Everything else must be distinct, exactly as the V1 ceremony demanded.
fn require_distinct_for_succession(accounts: &[AccountInfo<'_>]) -> Result<(), CoreSbfError> {
    const PERSON_SLOTS: [usize; 4] = [0, 4, 15, 18];
    for (left_index, left) in accounts.iter().enumerate() {
        for (right_index, right) in accounts
            .iter()
            .enumerate()
            .skip(left_index.saturating_add(1))
        {
            if left.key != right.key {
                continue;
            }
            let persons = PERSON_SLOTS.contains(&left_index) && PERSON_SLOTS.contains(&right_index);
            let both_system = left.key == &system_program::ID;
            if !persons && !both_system {
                return Err(CoreSbfError::AccountFrame);
            }
        }
    }
    Ok(())
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
