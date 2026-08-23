// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    AdapterCatalogV2, AdapterReleaseV2, CollateralPolicyV2, Error, Id, Result,
};

/// Market-owned references required to resolve collateral semantics.
///
/// Mint, decimals, token program, deployment, and adapter release deliberately
/// do not appear here: the immutable Realm/Profile/policy chain owns them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketCollateralBindingV2 {
    /// Canonical Market identity.
    pub market: Id,
    /// Immutable Realm selected at Market creation.
    pub realm: Id,
    /// Immutable parent Profile selected at Market creation.
    pub profile: Id,
    /// Per-Market cap frozen in Terms, in raw collateral atoms.
    pub collateral_cap_atoms: u64,
    /// Canonical authority PDA that owns collateral custody accounts.
    pub hoard_authority: Id,
    /// Canonical Market Hoard token-account address.
    pub hoard_token_account: Id,
}

/// Realm-owned parent Profile reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealmCollateralBindingV2 {
    /// Canonical Realm identity.
    pub realm: Id,
    /// Immutable parent Profile identity.
    pub profile: Id,
}

/// Profile-owned policy and adapter-release commitments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileCollateralBindingV2 {
    /// Canonical parent Profile identity.
    pub profile: Id,
    /// Recomputed canonical V2 policy identity.
    pub collateral_policy: Id,
    /// Exact release identity copied from the canonical V2 policy.
    pub adapter_release: Id,
}

/// Deployment/code facts produced by the live loader and release-manifest seam.
///
/// This crate checks equality but cannot authenticate upgradeable-loader account
/// provenance by itself. A caller assertion must never be used to construct
/// these facts in a live adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeReleaseObservationV2 {
    /// Presented executable token program account.
    pub token_program: Id,
    /// Whether the runtime marked the token program account executable.
    pub token_program_executable: bool,
    /// Token program is a read-only CPI target.
    pub token_program_writable: bool,
    /// Token program must not be a transaction signer.
    pub token_program_signer: bool,
    /// Digest recomputed from the authenticated external deployment manifest.
    pub token_program_deployment: Id,
    /// Digest of the executing parser/CPI implementation in this Clutch build.
    pub parser_cpi_code: Id,
}

/// Fully joined immutable collateral profile safe for parser and intent APIs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundCollateralProfileV2 {
    market: MarketCollateralBindingV2,
    policy_id: Id,
    policy: CollateralPolicyV2,
    release: AdapterReleaseV2,
}

impl BoundCollateralProfileV2 {
    /// Canonical Market binding.
    pub const fn market(self) -> MarketCollateralBindingV2 {
        self.market
    }

    /// Recomputed canonical policy identity.
    pub const fn policy_id(self) -> Id {
        self.policy_id
    }

    /// Immutable Realm-selected collateral policy.
    pub const fn policy(self) -> CollateralPolicyV2 {
        self.policy
    }

    /// Exact release resolved from the compiled catalog.
    pub const fn release(self) -> AdapterReleaseV2 {
        self.release
    }
}

/// Resolve one complete Market → Realm → Profile → policy → release chain.
pub fn bind_collateral_profile_v2(
    market: MarketCollateralBindingV2,
    realm: RealmCollateralBindingV2,
    profile: ProfileCollateralBindingV2,
    policy: CollateralPolicyV2,
    catalog: AdapterCatalogV2,
    runtime: RuntimeReleaseObservationV2,
) -> Result<BoundCollateralProfileV2> {
    for identity in [
        market.market,
        market.realm,
        market.profile,
        market.hoard_authority,
        market.hoard_token_account,
        realm.realm,
        realm.profile,
        profile.profile,
        profile.collateral_policy,
        profile.adapter_release,
        runtime.token_program,
        runtime.token_program_deployment,
        runtime.parser_cpi_code,
    ] {
        identity.require_live()?;
    }
    if market.realm != realm.realm
        || market.profile != realm.profile
        || realm.profile != profile.profile
    {
        return Err(Error::MismatchedBinding);
    }
    if market.market == market.realm
        || market.market == market.profile
        || market.market == market.hoard_authority
        || market.market == market.hoard_token_account
        || market.hoard_authority == market.hoard_token_account
    {
        return Err(Error::WrongAccountRole);
    }
    policy.admit_market_cap(market.collateral_cap_atoms)?;
    let policy_id = policy.id()?;
    if profile.collateral_policy != policy_id
        || profile.adapter_release != policy.adapter_release
    {
        return Err(Error::MismatchedBinding);
    }
    let release = catalog.resolve(policy.adapter_release)?;
    policy.validate_for_release(&release)?;
    if !runtime.token_program_executable
        || runtime.token_program_writable
        || runtime.token_program_signer
    {
        return Err(Error::WrongAccountRole);
    }
    if runtime.token_program != release.token_program {
        return Err(Error::WrongProgram);
    }
    if runtime.token_program_deployment != release.token_program_deployment
        || runtime.parser_cpi_code != release.parser_cpi_code
    {
        return Err(Error::MismatchedBinding);
    }
    Ok(BoundCollateralProfileV2 {
        market,
        policy_id,
        policy,
        release,
    })
}
