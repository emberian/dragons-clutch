//! Closed collateral-release catalog and Realm/Profile V2 admission.
//!
//! No caller supplies a release row or deployment/code identity. This ELF
//! contains local-real Token-2022 and legacy SPL rows pinned to the exact
//! program binaries installed by the repository's local validator profile.
//! Public-cluster deployments require separately reviewed rows and release
//! manifests.

use clutch_collateral_adapter_v2::{
    bind_realm_collateral_v2, AdapterCatalogV2, AdapterReleaseV2, BoundRealmCollateralV2,
    CollateralPolicyV2, Id, ProfileCollateralBindingV2, RealmCollateralBindingV2,
    RuntimeReleaseObservationV2, ADAPTER_RELEASE_V2_BYTES, COLLATERAL_POLICY_V2_BYTES,
};
use clutch_solana_layout::{account_len, Hash32, ProfileAccount, RealmAccount, PROFILE_SCHEMA_V2};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::loader_state::{
    decode_loader_pair_v1, decode_synthesized_genesis_loader_pair_v1, LoaderAccountViewV1,
    UpgradeAuthorityV1, PROGRAMDATA_METADATA_LEN,
};
use crate::seeds;

const COLLATERAL_RELEASE_DEPLOYMENT_RECEIPT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/collateral-release/deployment-locus/v2\0";

/// Exact runtime locus at which a compiled release's ProgramData is admitted.
///
/// The local validator's genesis synthesizer writes a noncanonical-for-source
/// `Some(Pubkey::default())` authority at slot zero. Public observed releases
/// use a positive deployment slot and the generic canonical loader decoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum CollateralDeploymentLocusV2 {
    /// Repository-controlled `--bpf-program` genesis deployment.
    SynthesizedGenesisZero = 1,
    /// Positive-slot observed ProgramData on a public or persistent cluster.
    ObservedPositive = 2,
}

impl CollateralDeploymentLocusV2 {
    const fn wire(self) -> u8 {
        match self {
            Self::SynthesizedGenesisZero => 1,
            Self::ObservedPositive => 2,
        }
    }
}

/// Authority state accepted by the collateral-specific deployment proof.
/// The synthesized default variant is never returned by the generic Source
/// loader decoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CollateralUpgradeAuthorityV2 {
    /// Local genesis `Some(Pubkey::default())` sentinel.
    SynthesizedDefault,
    /// Canonical absent authority on an observed positive-slot deployment.
    Immutable,
    /// Canonical nonzero authority on an observed positive-slot deployment.
    Present(Id),
}

/// SHA-256 of `spl_token_2022-10.0.0.so` from
/// `solana-program-binaries 4.2.1`, the binary installed by the local-real
/// validator profile.
pub const LOCAL_REAL_TOKEN_2022_DEPLOYMENT_ID_V2: Id = Id::from_bytes([
    0xa7, 0x94, 0x16, 0x14, 0x08, 0x08, 0x0f, 0x69, 0x0d, 0xac, 0x00, 0x83, 0x2f, 0x45, 0xb3, 0xc3,
    0xe2, 0xb7, 0x1f, 0x13, 0x39, 0x58, 0x66, 0x67, 0xad, 0x1f, 0x97, 0x9c, 0xf9, 0x1d, 0x5b, 0x68,
]);

/// SHA-256 identity of the frozen parser/CPI capability manifest
/// `dragons-clutch/collateral-adapter-v2/parser-cpi/v2/token2022-base/2026-08-23`.
pub const COLLATERAL_PARSER_CPI_CODE_ID_V2: Id = Id::from_bytes([
    0x2c, 0xaf, 0x70, 0xb7, 0x44, 0x4d, 0x42, 0x8b, 0x14, 0x8a, 0xc9, 0xf0, 0x35, 0x6c, 0x00, 0x53,
    0x62, 0xc1, 0x87, 0xfc, 0x79, 0xab, 0x70, 0xf7, 0xc5, 0x3f, 0xa5, 0x88, 0x72, 0x9c, 0x66, 0xe8,
]);

/// SHA-256 of `spl_p_token-1.0.0.so` from `solana-program-binaries` 4.2.1,
/// installed at the legacy SPL Token address by the local-real validator.
pub const LOCAL_REAL_LEGACY_SPL_DEPLOYMENT_ID_V2: Id = Id::from_bytes([
    0x81, 0x90, 0xd3, 0xf7, 0xce, 0xb6, 0xcb, 0x7a, 0x7a, 0x8d, 0x89, 0x24, 0xbf, 0xf8, 0x9f, 0x9f,
    0x61, 0x1e, 0x15, 0xce, 0x1f, 0x80, 0x6f, 0x2b, 0x62, 0x37, 0xf3, 0x31, 0x1a, 0x98, 0xf6, 0x97,
]);

/// SHA-256 identity of the frozen legacy SPL parser/CPI capability manifest
/// `dragons-clutch/collateral-adapter-v2/parser-cpi/v2/legacy-spl/2026-08-23`.
pub const COLLATERAL_LEGACY_SPL_PARSER_CPI_CODE_ID_V2: Id = Id::from_bytes([
    0x85, 0xca, 0x0f, 0x3a, 0x32, 0xd0, 0xbe, 0x09, 0x84, 0x2e, 0x07, 0xf6, 0x3e, 0x88, 0x62, 0x04,
    0xe6, 0xd6, 0x07, 0xce, 0xd5, 0x16, 0x21, 0x77, 0x76, 0x22, 0x81, 0x30, 0xef, 0x6b, 0x27, 0x77,
]);

/// Token-2022 collateral release compiled into the current local-real ELF.
pub const LOCAL_REAL_TOKEN_2022_RELEASE_V2: AdapterReleaseV2 = AdapterReleaseV2::token_2022_base(
    LOCAL_REAL_TOKEN_2022_DEPLOYMENT_ID_V2,
    COLLATERAL_PARSER_CPI_CODE_ID_V2,
);

/// Local-real legacy SPL release under the narrower PDA-sole-signer guard.
pub const LOCAL_REAL_LEGACY_SPL_RELEASE_V2: AdapterReleaseV2 = AdapterReleaseV2::legacy_spl(
    LOCAL_REAL_LEGACY_SPL_DEPLOYMENT_ID_V2,
    COLLATERAL_LEGACY_SPL_PARSER_CPI_CODE_ID_V2,
);

#[cfg(all(
    feature = "laboratory-fixtures",
    feature = "observed-positive-collateral-release-manifest"
))]
compile_error!(
    "laboratory synthesized collateral and observed-positive release manifests are distinct ELFs"
);

/// Checked release-manifest row selected by this exact program build.
///
/// Observed-positive rows pin every mutable loader coordinate in addition to
/// the immutable ELF identity already owned by [`AdapterReleaseV2`]. The
/// synthesized local row deliberately leaves ProgramData zero because the
/// exact linked account is authenticated from the local genesis Program body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompiledCollateralReleaseManifestV2 {
    release: AdapterReleaseV2,
    locus: CollateralDeploymentLocusV2,
    expected_programdata_account: Id,
    expected_deployment_slot: u64,
    expected_upgrade_authority: CollateralUpgradeAuthorityV2,
}

impl CompiledCollateralReleaseManifestV2 {
    /// Construct one checked positive-slot row in a repository-owned release
    /// manifest. Runtime validation still rejects zero identities/slot and any
    /// mismatch against the linked loader accounts.
    pub(crate) const fn observed_positive(
        release: AdapterReleaseV2,
        programdata_account: Id,
        deployment_slot: u64,
        upgrade_authority: CollateralUpgradeAuthorityV2,
    ) -> Self {
        Self {
            release,
            locus: CollateralDeploymentLocusV2::ObservedPositive,
            expected_programdata_account: programdata_account,
            expected_deployment_slot: deployment_slot,
            expected_upgrade_authority: upgrade_authority,
        }
    }
}

#[cfg(feature = "laboratory-fixtures")]
static COMPILED_COLLATERAL_RELEASES_V2: [AdapterReleaseV2; 2] = [
    LOCAL_REAL_TOKEN_2022_RELEASE_V2,
    LOCAL_REAL_LEGACY_SPL_RELEASE_V2,
];
#[cfg(feature = "laboratory-fixtures")]
static COMPILED_COLLATERAL_RELEASE_MANIFESTS_V2: [CompiledCollateralReleaseManifestV2; 2] = [
    CompiledCollateralReleaseManifestV2 {
        release: LOCAL_REAL_TOKEN_2022_RELEASE_V2,
        locus: CollateralDeploymentLocusV2::SynthesizedGenesisZero,
        expected_programdata_account: Id::ZERO,
        expected_deployment_slot: 0,
        expected_upgrade_authority: CollateralUpgradeAuthorityV2::SynthesizedDefault,
    },
    CompiledCollateralReleaseManifestV2 {
        release: LOCAL_REAL_LEGACY_SPL_RELEASE_V2,
        locus: CollateralDeploymentLocusV2::SynthesizedGenesisZero,
        expected_programdata_account: Id::ZERO,
        expected_deployment_slot: 0,
        expected_upgrade_authority: CollateralUpgradeAuthorityV2::SynthesizedDefault,
    },
];

#[cfg(all(
    not(feature = "laboratory-fixtures"),
    not(feature = "observed-positive-collateral-release-manifest")
))]
static COMPILED_COLLATERAL_RELEASES_V2: [AdapterReleaseV2; 0] = [];

#[cfg(all(
    not(feature = "laboratory-fixtures"),
    not(feature = "observed-positive-collateral-release-manifest")
))]
static COMPILED_COLLATERAL_RELEASE_MANIFESTS_V2: [CompiledCollateralReleaseManifestV2; 0] = [];

#[cfg(feature = "observed-positive-collateral-release-manifest")]
fn compiled_collateral_releases_v2() -> &'static [AdapterReleaseV2] {
    &crate::observed_collateral_release_manifest_v2::OBSERVED_COLLATERAL_RELEASES_V2
}

#[cfg(not(feature = "observed-positive-collateral-release-manifest"))]
fn compiled_collateral_releases_v2() -> &'static [AdapterReleaseV2] {
    &COMPILED_COLLATERAL_RELEASES_V2
}

#[cfg(feature = "observed-positive-collateral-release-manifest")]
fn compiled_collateral_release_manifests_v2(
) -> &'static [CompiledCollateralReleaseManifestV2] {
    &crate::observed_collateral_release_manifest_v2::OBSERVED_COLLATERAL_RELEASE_MANIFESTS_V2
}

#[cfg(not(feature = "observed-positive-collateral-release-manifest"))]
fn compiled_collateral_release_manifests_v2(
) -> &'static [CompiledCollateralReleaseManifestV2] {
    &COMPILED_COLLATERAL_RELEASE_MANIFESTS_V2
}

const _: () = assert!(ADAPTER_RELEASE_V2_BYTES == 192);

/// Return the closed release catalog compiled into this program.
///
/// The local-real laboratory ELFs have binary-pinned Token-2022 and legacy SPL
/// rows. The Structured custody laboratory selects those runtime rows directly
/// without compiling fixture-account builders. Default and public-cluster
/// artifacts have no rows unless that exact ELF selects the separately reviewed
/// observed-positive manifest. The checked repository manifest is currently
/// empty and never accepts build-time environment rows.
pub fn compiled_collateral_catalog_v2() -> Outcome<AdapterCatalogV2> {
    validate_compiled_collateral_release_manifest_v2(
        compiled_collateral_releases_v2(),
        compiled_collateral_release_manifests_v2(),
    )?;
    AdapterCatalogV2::new(compiled_collateral_releases_v2())
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))
}

fn validate_compiled_collateral_release_manifest_v2(
    releases: &[AdapterReleaseV2],
    manifests: &[CompiledCollateralReleaseManifestV2],
) -> Outcome<()> {
    require(
        releases.len() == manifests.len(),
        ClutchError::AuthorizationUnavailable,
    )?;
    for release in releases {
        let mut found = false;
        for manifest in manifests {
            if manifest.release == *release {
                require(!found, ClutchError::AuthorizationUnavailable)?;
                found = true;
            }
        }
        require(found, ClutchError::AuthorizationUnavailable)?;
    }
    for manifest in manifests {
        require(
            releases.iter().any(|release| *release == manifest.release)
                && match manifest.locus {
                    CollateralDeploymentLocusV2::SynthesizedGenesisZero => {
                        manifest.expected_programdata_account.is_zero()
                            && manifest.expected_deployment_slot == 0
                            && manifest.expected_upgrade_authority
                                == CollateralUpgradeAuthorityV2::SynthesizedDefault
                    }
                    CollateralDeploymentLocusV2::ObservedPositive => {
                        !manifest.expected_programdata_account.is_zero()
                            && manifest.expected_deployment_slot != 0
                            && match manifest.expected_upgrade_authority {
                                CollateralUpgradeAuthorityV2::Immutable => true,
                                CollateralUpgradeAuthorityV2::Present(authority) => {
                                    !authority.is_zero()
                                }
                                CollateralUpgradeAuthorityV2::SynthesizedDefault => false,
                            }
                    }
                },
            ClutchError::AuthorizationUnavailable,
        )?;
    }
    Ok(())
}

fn compiled_collateral_release_manifest_v2(
    release: AdapterReleaseV2,
) -> Outcome<CompiledCollateralReleaseManifestV2> {
    validate_compiled_collateral_release_manifest_v2(
        compiled_collateral_releases_v2(),
        compiled_collateral_release_manifests_v2(),
    )?;
    let manifests = compiled_collateral_release_manifests_v2();
    require(
        compiled_collateral_releases_v2()
            .iter()
            .any(|row| *row == release),
        ClutchError::AuthorizationUnavailable,
    )?;
    for manifest in manifests {
        if manifest.release == release {
            return Ok(*manifest);
        }
    }
    Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
}

/// Private runtime proof that one compiled collateral release is backed by
/// the exact Upgradeable Loader deployment and explicit locus it names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedCollateralReleaseDeploymentV2 {
    release: AdapterReleaseV2,
    release_id: Id,
    programdata_account: Id,
    deployment_slot: u64,
    locus: CollateralDeploymentLocusV2,
    upgrade_authority: CollateralUpgradeAuthorityV2,
    receipt_id: Id,
}

impl AuthenticatedCollateralReleaseDeploymentV2 {
    /// Exact compiled release whose deployment bytes were authenticated.
    pub(crate) const fn release(self) -> AdapterReleaseV2 {
        self.release
    }

    /// Canonical content identity persisted by Profile V2.
    pub(crate) const fn release_id(self) -> Id {
        self.release_id
    }

    /// Exact ProgramData account linked by the executable program account.
    pub(crate) const fn programdata_account(self) -> Id {
        self.programdata_account
    }

    /// Loader-recorded deployment slot. Immutable code identity, not this
    /// mutable coordinate, is the release owner.
    pub(crate) const fn deployment_slot(self) -> u64 {
        self.deployment_slot
    }

    /// Exact compiled admission locus.
    pub(crate) const fn locus(self) -> CollateralDeploymentLocusV2 {
        self.locus
    }

    /// Authority state observed at this value-bearing boundary.
    pub(crate) const fn upgrade_authority(self) -> CollateralUpgradeAuthorityV2 {
        self.upgrade_authority
    }

    /// Private receipt suitable for Product/Profile founding joins.
    pub(crate) const fn receipt_id(self) -> Id {
        self.receipt_id
    }
}

/// Authenticate one compiled release at its catalog-selected runtime locus.
///
/// This proof is intentionally re-run at every value-bearing collateral CPI;
/// Profile creation records selection, not permanent trust in mutable loader
/// state.
pub(crate) fn authenticate_collateral_release_deployment_v2(
    release: AdapterReleaseV2,
    token_program: &AccountInfo<'_>,
    token_programdata: &AccountInfo<'_>,
) -> Outcome<AuthenticatedCollateralReleaseDeploymentV2> {
    let manifest = compiled_collateral_release_manifest_v2(release)?;
    authenticate_collateral_release_deployment_at_manifest_v2(
        release,
        manifest,
        token_program,
        token_programdata,
    )
}

fn authenticate_collateral_release_deployment_at_manifest_v2(
    release: AdapterReleaseV2,
    manifest: CompiledCollateralReleaseManifestV2,
    token_program: &AccountInfo<'_>,
    token_programdata: &AccountInfo<'_>,
) -> Outcome<AuthenticatedCollateralReleaseDeploymentV2> {
    release
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    require(manifest.release == release, ClutchError::AuthorizationUnavailable)?;
    require(
        token_program.key != token_programdata.key
            && token_program.key.to_bytes() == release.token_program.bytes()
            && !token_program.is_signer
            && !token_program.is_writable
            && token_program.executable
            && !token_programdata.is_signer
            && !token_programdata.is_writable
            && !token_programdata.executable,
        ClutchError::MismatchedState,
    )?;
    let program_data = token_program
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let programdata_data = token_programdata
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(
        programdata_data.len() > PROGRAMDATA_METADATA_LEN,
        ClutchError::AuthorizationUnavailable,
    )?;
    let program_view = LoaderAccountViewV1::new(
        token_program.key.to_bytes(),
        token_program.owner.to_bytes(),
        token_program.executable,
        &program_data,
    );
    let programdata_view = LoaderAccountViewV1::new(
        token_programdata.key.to_bytes(),
        token_programdata.owner.to_bytes(),
        token_programdata.executable,
        &programdata_data,
    );
    let (deployment_slot, upgrade_authority) = match manifest.locus {
        CollateralDeploymentLocusV2::SynthesizedGenesisZero => {
            let loader = decode_synthesized_genesis_loader_pair_v1(program_view, programdata_view)
                .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
            require(
                loader.linked_programdata == token_programdata.key.to_bytes()
                    && loader.deployment_slot == 0,
                ClutchError::AuthorizationUnavailable,
            )?;
            (0, CollateralUpgradeAuthorityV2::SynthesizedDefault)
        }
        CollateralDeploymentLocusV2::ObservedPositive => {
            let loader = decode_loader_pair_v1(program_view, programdata_view)
                .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
            require(
                loader.state.deployment_slot != 0,
                ClutchError::AuthorizationUnavailable,
            )?;
            let authority = match loader.upgrade_authority {
                UpgradeAuthorityV1::Immutable => CollateralUpgradeAuthorityV2::Immutable,
                UpgradeAuthorityV1::Present(authority) => {
                    CollateralUpgradeAuthorityV2::Present(Id::from_bytes(authority))
                }
            };
            require(
                loader.state.linked_programdata
                    == manifest.expected_programdata_account.bytes()
                    && loader.state.deployment_slot == manifest.expected_deployment_slot
                    && authority == manifest.expected_upgrade_authority,
                ClutchError::AuthorizationUnavailable,
            )?;
            (loader.state.deployment_slot, authority)
        }
    };
    let deployment_digest = Id::from_bytes(
        solana_sha256_hasher::hashv(&[&programdata_data[PROGRAMDATA_METADATA_LEN..]]).to_bytes(),
    );
    require(
        deployment_digest == release.token_program_deployment,
        ClutchError::AuthorizationUnavailable,
    )?;
    let release_id = release
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let (authority_tag, authority_bytes) = match upgrade_authority {
        CollateralUpgradeAuthorityV2::SynthesizedDefault => (2u8, [0u8; 32]),
        CollateralUpgradeAuthorityV2::Immutable => (0u8, [0u8; 32]),
        CollateralUpgradeAuthorityV2::Present(authority) => (1u8, authority.bytes()),
    };
    let locus_byte = [manifest.locus.wire()];
    let authority_tag = [authority_tag];
    let receipt_id = Id::from_bytes(
        solana_sha256_hasher::hashv(&[
            COLLATERAL_RELEASE_DEPLOYMENT_RECEIPT_DOMAIN_V2,
            &release_id.bytes(),
            &token_program.key.to_bytes(),
            &token_programdata.key.to_bytes(),
            &deployment_slot.to_le_bytes(),
            &locus_byte,
            &authority_tag,
            &authority_bytes,
            &deployment_digest.bytes(),
        ])
        .to_bytes(),
    );
    receipt_id
        .require_live()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    Ok(AuthenticatedCollateralReleaseDeploymentV2 {
        release,
        release_id,
        programdata_account: Id::from_bytes(token_programdata.key.to_bytes()),
        deployment_slot,
        locus: manifest.locus,
        upgrade_authority,
        receipt_id,
    })
}

/// Authenticate the live Realm→ProfileV2→CollateralPolicyV2→compiled-release
/// chain without accepting any caller-shaped release or deployment identity.
pub(crate) fn authenticate_realm_collateral_v2(
    program_id: &Pubkey,
    realm_account: &AccountInfo<'_>,
    profile_account: &AccountInfo<'_>,
    policy_account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
) -> Outcome<BoundRealmCollateralV2> {
    require_read_only_program_account(program_id, realm_account, account_len::REALM)?;
    require_read_only_program_account(program_id, profile_account, account_len::PROFILE)?;
    require_read_only_program_account(program_id, policy_account, COLLATERAL_POLICY_V2_BYTES)?;
    require(
        !token_program.is_writable && !token_program.is_signer && token_program.executable,
        ClutchError::MismatchedState,
    )?;

    let realm = RealmAccount::decode(&realm_account.data.borrow())?;
    let profile = ProfileAccount::decode(&profile_account.data.borrow())?;
    require(
        realm.profile_version == PROFILE_SCHEMA_V2
            && profile.version == PROFILE_SCHEMA_V2
            && profile.realm == realm.realm
            && profile.profile == realm.profile,
        ClutchError::MismatchedState,
    )?;
    let realm_bytes = realm.realm.bytes();
    let profile_bytes = profile.profile.bytes();
    expect_pda(
        realm_account.key,
        seeds::realm_pda(program_id, &realm_bytes),
        Some(realm.stored_bump),
    )?;
    expect_pda(
        profile_account.key,
        seeds::profile_pda(program_id, &realm_bytes, &profile_bytes),
        None,
    )?;

    let policy = CollateralPolicyV2::decode(&policy_account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let policy_id = policy
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        Hash32::from_bytes(policy_id.bytes()) == profile.collateral_policy_id
            && Hash32::from_bytes(policy.adapter_release.bytes()) == profile.adapter_release_id,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        policy_account.key,
        seeds::policy_pda(program_id, &profile_bytes, &policy_id.bytes()),
        None,
    )?;

    let catalog = compiled_collateral_catalog_v2()?;
    let selected = catalog
        .resolve(policy.adapter_release)
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    require(
        token_program.key.to_bytes() == selected.token_program.bytes(),
        ClutchError::MismatchedState,
    )?;
    bind_realm_collateral_v2(
        RealmCollateralBindingV2 {
            realm: Id::from_bytes(realm.realm.bytes()),
            profile: Id::from_bytes(profile.profile.bytes()),
        },
        ProfileCollateralBindingV2 {
            profile: Id::from_bytes(profile.profile.bytes()),
            collateral_policy: Id::from_bytes(profile.collateral_policy_id.bytes()),
            adapter_release: Id::from_bytes(profile.adapter_release_id.bytes()),
        },
        policy,
        catalog,
        RuntimeReleaseObservationV2 {
            token_program: Id::from_bytes(token_program.key.to_bytes()),
            token_program_executable: token_program.executable,
            token_program_writable: token_program.is_writable,
            token_program_signer: token_program.is_signer,
            token_program_deployment: selected.token_program_deployment,
            parser_cpi_code: selected.parser_cpi_code,
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))
}

fn require_read_only_program_account(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    exact_len: usize,
) -> Outcome<()> {
    require(
        *account.owner == *program_id,
        ClutchError::WrongProgramOwner,
    )?;
    require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(
        account.data_len() == exact_len,
        ClutchError::WrongDataLength,
    )
}

#[cfg(test)]
mod deployment_release_tests {
    use super::*;
    use crate::loader_state::UPGRADEABLE_LOADER_ID;
    use solana_pubkey::Pubkey;

    const ELF: &[u8] = b"exact-test-token-program-elf";

    struct Cell {
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        data: Vec<u8>,
        executable: bool,
    }

    impl Cell {
        fn info(&mut self) -> AccountInfo<'_> {
            AccountInfo::new(
                &self.key,
                false,
                false,
                &mut self.lamports,
                &mut self.data,
                &self.owner,
                self.executable,
            )
        }
    }

    fn deployment_id(elf: &[u8]) -> Id {
        Id::from_bytes(solana_sha256_hasher::hashv(&[elf]).to_bytes())
    }

    fn release(token_2022: bool) -> AdapterReleaseV2 {
        if token_2022 {
            AdapterReleaseV2::token_2022_base(deployment_id(ELF), Id::from_bytes([91; 32]))
        } else {
            AdapterReleaseV2::legacy_spl(deployment_id(ELF), Id::from_bytes([92; 32]))
        }
    }

    fn program_bytes(programdata: Pubkey) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&2_u32.to_le_bytes());
        bytes.extend_from_slice(&programdata.to_bytes());
        bytes
    }

    fn programdata_bytes(slot: u64, authority: Option<[u8; 32]>, elf: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0u8; PROGRAMDATA_METADATA_LEN];
        bytes[..4].copy_from_slice(&3_u32.to_le_bytes());
        bytes[4..12].copy_from_slice(&slot.to_le_bytes());
        match authority {
            None => bytes[12] = 0,
            Some(authority) => {
                bytes[12] = 1;
                bytes[13..45].copy_from_slice(&authority);
            }
        }
        bytes.extend_from_slice(elf);
        bytes
    }

    fn cells(
        release: AdapterReleaseV2,
        slot: u64,
        authority: Option<[u8; 32]>,
        elf: &[u8],
    ) -> (Cell, Cell) {
        let programdata_key = Pubkey::new_from_array([41; 32]);
        let loader = Pubkey::new_from_array(UPGRADEABLE_LOADER_ID);
        (
            Cell {
                key: Pubkey::new_from_array(release.token_program.bytes()),
                owner: loader,
                lamports: 1,
                data: program_bytes(programdata_key),
                executable: true,
            },
            Cell {
                key: programdata_key,
                owner: loader,
                lamports: 1,
                data: programdata_bytes(slot, authority, elf),
                executable: false,
            },
        )
    }

    fn manifest(
        release: AdapterReleaseV2,
        locus: CollateralDeploymentLocusV2,
        slot: u64,
        authority: CollateralUpgradeAuthorityV2,
    ) -> CompiledCollateralReleaseManifestV2 {
        CompiledCollateralReleaseManifestV2 {
            release,
            locus,
            expected_programdata_account: if locus
                == CollateralDeploymentLocusV2::ObservedPositive
            {
                Id::from_bytes([41; 32])
            } else {
                Id::ZERO
            },
            expected_deployment_slot: slot,
            expected_upgrade_authority: authority,
        }
    }

    #[test]
    fn both_token_families_accept_only_the_exact_synthesized_genesis_locus() {
        for token_2022 in [false, true] {
            let release = release(token_2022);
            let (mut program, mut programdata) = cells(release, 0, Some([0; 32]), ELF);
            let program_info = program.info();
            let programdata_info = programdata.info();
            let accepted = authenticate_collateral_release_deployment_at_manifest_v2(
                release,
                manifest(
                    release,
                    CollateralDeploymentLocusV2::SynthesizedGenesisZero,
                    0,
                    CollateralUpgradeAuthorityV2::SynthesizedDefault,
                ),
                &program_info,
                &programdata_info,
            )
            .unwrap();
            assert_eq!(accepted.release(), release);
            assert_eq!(accepted.release_id(), release.id().unwrap());
            assert_eq!(accepted.deployment_slot(), 0);
            assert_eq!(accepted.programdata_account(), Id::from_bytes([41; 32]));
            assert_eq!(
                accepted.locus(),
                CollateralDeploymentLocusV2::SynthesizedGenesisZero
            );
            assert_eq!(
                accepted.upgrade_authority(),
                CollateralUpgradeAuthorityV2::SynthesizedDefault
            );
        }
    }

    #[test]
    fn observed_positive_accepts_canonical_present_or_absent_authority() {
        let release = release(true);
        for authority in [None, Some([73; 32])] {
            let (mut program, mut programdata) = cells(release, 17, authority, ELF);
            let program_info = program.info();
            let programdata_info = programdata.info();
            let expected_authority = match authority {
                None => CollateralUpgradeAuthorityV2::Immutable,
                Some(authority) => {
                    CollateralUpgradeAuthorityV2::Present(Id::from_bytes(authority))
                }
            };
            let accepted = authenticate_collateral_release_deployment_at_manifest_v2(
                release,
                manifest(
                    release,
                    CollateralDeploymentLocusV2::ObservedPositive,
                    17,
                    expected_authority,
                ),
                &program_info,
                &programdata_info,
            )
            .unwrap();
            assert_eq!(accepted.deployment_slot(), 17);
            match authority {
                None => assert_eq!(
                    accepted.upgrade_authority(),
                    CollateralUpgradeAuthorityV2::Immutable
                ),
                Some(authority) => assert_eq!(
                    accepted.upgrade_authority(),
                    CollateralUpgradeAuthorityV2::Present(Id::from_bytes(authority))
                ),
            }
        }
    }

    #[test]
    fn observed_manifest_refuses_stale_slot_authority_and_programdata() {
        let release = release(true);
        let (mut program, mut programdata) = cells(release, 17, Some([73; 32]), ELF);
        let program_info = program.info();
        let programdata_info = programdata.info();
        for hostile in [
            CompiledCollateralReleaseManifestV2::observed_positive(
                release,
                Id::from_bytes([41; 32]),
                18,
                CollateralUpgradeAuthorityV2::Present(Id::from_bytes([73; 32])),
            ),
            CompiledCollateralReleaseManifestV2::observed_positive(
                release,
                Id::from_bytes([41; 32]),
                17,
                CollateralUpgradeAuthorityV2::Immutable,
            ),
            CompiledCollateralReleaseManifestV2::observed_positive(
                release,
                Id::from_bytes([42; 32]),
                17,
                CollateralUpgradeAuthorityV2::Present(Id::from_bytes([73; 32])),
            ),
        ] {
            assert!(authenticate_collateral_release_deployment_at_manifest_v2(
                release,
                hostile,
                &program_info,
                &programdata_info,
            )
            .is_err());
        }
    }

    #[test]
    fn compiled_manifest_refuses_duplicate_missing_and_wrong_locus_rows() {
        let token_2022 = release(true);
        let legacy = release(false);
        let token_2022_manifest = manifest(
            token_2022,
            CollateralDeploymentLocusV2::ObservedPositive,
            17,
            CollateralUpgradeAuthorityV2::Immutable,
        );
        let legacy_manifest = manifest(
            legacy,
            CollateralDeploymentLocusV2::ObservedPositive,
            17,
            CollateralUpgradeAuthorityV2::Immutable,
        );
        assert!(validate_compiled_collateral_release_manifest_v2(
            &[token_2022, legacy],
            &[token_2022_manifest, legacy_manifest],
        )
        .is_ok());
        assert!(validate_compiled_collateral_release_manifest_v2(
            &[token_2022, legacy],
            &[token_2022_manifest, token_2022_manifest],
        )
        .is_err());
        assert!(validate_compiled_collateral_release_manifest_v2(
            &[token_2022],
            &[],
        )
        .is_err());
        assert!(validate_compiled_collateral_release_manifest_v2(
            &[token_2022],
            &[CompiledCollateralReleaseManifestV2 {
                release: token_2022,
                locus: CollateralDeploymentLocusV2::ObservedPositive,
                expected_programdata_account: Id::ZERO,
                expected_deployment_slot: 17,
                expected_upgrade_authority: CollateralUpgradeAuthorityV2::Immutable,
            }],
        )
        .is_err());
    }

    #[test]
    fn wrong_locus_bytes_and_programdata_substitution_refuse() {
        let release = release(true);

        let (mut program, mut wrong_synth_authority) = cells(release, 0, None, ELF);
        let program_info = program.info();
        let wrong_synth_authority_info = wrong_synth_authority.info();
        assert!(authenticate_collateral_release_deployment_at_manifest_v2(
            release,
            manifest(
                release,
                CollateralDeploymentLocusV2::SynthesizedGenesisZero,
                0,
                CollateralUpgradeAuthorityV2::SynthesizedDefault,
            ),
            &program_info,
            &wrong_synth_authority_info,
        )
        .is_err());

        let (mut program, mut zero_observed) = cells(release, 0, Some([73; 32]), ELF);
        let program_info = program.info();
        let zero_observed_info = zero_observed.info();
        assert!(authenticate_collateral_release_deployment_at_manifest_v2(
            release,
            manifest(
                release,
                CollateralDeploymentLocusV2::ObservedPositive,
                17,
                CollateralUpgradeAuthorityV2::Present(Id::from_bytes([73; 32])),
            ),
            &program_info,
            &zero_observed_info,
        )
        .is_err());

        let (mut program, mut wrong_bytes) = cells(release, 17, None, b"different-elf");
        let program_info = program.info();
        let wrong_bytes_info = wrong_bytes.info();
        assert!(authenticate_collateral_release_deployment_at_manifest_v2(
            release,
            manifest(
                release,
                CollateralDeploymentLocusV2::ObservedPositive,
                17,
                CollateralUpgradeAuthorityV2::Immutable,
            ),
            &program_info,
            &wrong_bytes_info,
        )
        .is_err());

        let (mut program, mut substituted) = cells(release, 17, None, ELF);
        substituted.key = Pubkey::new_from_array([42; 32]);
        let program_info = program.info();
        let substituted_info = substituted.info();
        assert!(authenticate_collateral_release_deployment_at_manifest_v2(
            release,
            manifest(
                release,
                CollateralDeploymentLocusV2::ObservedPositive,
                17,
                CollateralUpgradeAuthorityV2::Immutable,
            ),
            &program_info,
            &substituted_info,
        )
        .is_err());
    }
}
