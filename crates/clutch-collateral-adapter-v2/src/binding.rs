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

/// Realm/Profile-level collateral binding available before any Market exists.
///
/// This is the correct authority for Series funding vault construction and
/// transfers: it binds mint/program/deployment/decimals and both policy
/// ceilings without inventing a Market, Hoard, or Market cap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundRealmCollateralV2 {
    realm: RealmCollateralBindingV2,
    policy_id: Id,
    policy: CollateralPolicyV2,
    release: AdapterReleaseV2,
}

impl BoundRealmCollateralV2 {
    /// Canonical immutable Realm/Profile references.
    pub const fn realm(self) -> RealmCollateralBindingV2 {
        self.realm
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

/// Market refinement of an already joined immutable Realm collateral profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundCollateralProfileV2 {
    market: MarketCollateralBindingV2,
    realm: BoundRealmCollateralV2,
}

impl BoundCollateralProfileV2 {
    /// Canonical Market binding.
    pub const fn market(self) -> MarketCollateralBindingV2 {
        self.market
    }

    /// Recomputed canonical policy identity.
    pub const fn policy_id(self) -> Id {
        self.realm.policy_id
    }

    /// Immutable Realm-selected collateral policy.
    pub const fn policy(self) -> CollateralPolicyV2 {
        self.realm.policy
    }

    /// Exact release resolved from the compiled catalog.
    pub const fn release(self) -> AdapterReleaseV2 {
        self.realm.release
    }

    /// Realm/Profile-level binding refined by this Market.
    pub const fn realm_bound(self) -> BoundRealmCollateralV2 {
        self.realm
    }
}

/// Read-only projection shared by profile-level and Market-refined contexts.
///
/// Generic custody creation may consume either context because it needs no
/// Market fact. Hoard transfers intentionally do not use this trait and still
/// require [`BoundCollateralProfileV2`] explicitly.
pub trait RealmCollateralContextV2: Copy {
    /// Return the authenticated Realm/Profile-level collateral binding.
    fn realm_collateral(self) -> BoundRealmCollateralV2;
}

impl RealmCollateralContextV2 for BoundRealmCollateralV2 {
    fn realm_collateral(self) -> BoundRealmCollateralV2 {
        self
    }
}

impl RealmCollateralContextV2 for BoundCollateralProfileV2 {
    fn realm_collateral(self) -> BoundRealmCollateralV2 {
        self.realm
    }
}

/// Resolve the Realm → Profile → policy → release chain before any Market exists.
pub fn bind_realm_collateral_v2(
    realm: RealmCollateralBindingV2,
    profile: ProfileCollateralBindingV2,
    policy: CollateralPolicyV2,
    catalog: AdapterCatalogV2,
    runtime: RuntimeReleaseObservationV2,
) -> Result<BoundRealmCollateralV2> {
    for identity in [
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
    if realm.profile != profile.profile || realm.realm == realm.profile {
        return Err(Error::MismatchedBinding);
    }
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
    Ok(BoundRealmCollateralV2 {
        realm,
        policy_id,
        policy,
        release,
    })
}

/// Refine an authenticated Realm collateral profile with one concrete Market.
pub fn refine_market_collateral_v2(
    realm: BoundRealmCollateralV2,
    market: MarketCollateralBindingV2,
) -> Result<BoundCollateralProfileV2> {
    for identity in [
        market.market,
        market.realm,
        market.profile,
        market.hoard_authority,
        market.hoard_token_account,
    ] {
        identity.require_live()?;
    }
    if market.realm != realm.realm.realm
        || market.profile != realm.realm.profile
        || market.market == market.realm
        || market.market == market.profile
        || market.market == market.hoard_authority
        || market.market == market.hoard_token_account
        || market.hoard_authority == market.hoard_token_account
    {
        return Err(Error::MismatchedBinding);
    }
    realm.policy.admit_market_cap(market.collateral_cap_atoms)?;
    Ok(BoundCollateralProfileV2 { market, realm })
}

/// Resolve the complete Market chain as a compatibility convenience.
pub fn bind_collateral_profile_v2(
    market: MarketCollateralBindingV2,
    realm: RealmCollateralBindingV2,
    profile: ProfileCollateralBindingV2,
    policy: CollateralPolicyV2,
    catalog: AdapterCatalogV2,
    runtime: RuntimeReleaseObservationV2,
) -> Result<BoundCollateralProfileV2> {
    let realm = bind_realm_collateral_v2(realm, profile, policy, catalog, runtime)?;
    refine_market_collateral_v2(realm, market)
}
