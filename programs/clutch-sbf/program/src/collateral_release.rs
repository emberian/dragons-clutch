//! Closed collateral-release catalog and Realm/Profile V2 admission.
//!
//! No caller supplies a release row or deployment/code identity. This ELF
//! contains one local-real Token-2022 row pinned to the exact program binary
//! installed by the repository's local validator profile. Public-cluster
//! deployments require a separately reviewed row and release manifest.

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
use crate::seeds;

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

/// Sole collateral release compiled into the current local-real ELF.
pub const LOCAL_REAL_TOKEN_2022_RELEASE_V2: AdapterReleaseV2 = AdapterReleaseV2::token_2022_base(
    LOCAL_REAL_TOKEN_2022_DEPLOYMENT_ID_V2,
    COLLATERAL_PARSER_CPI_CODE_ID_V2,
);

#[cfg(feature = "laboratory-fixtures")]
static COMPILED_COLLATERAL_RELEASES_V2: [AdapterReleaseV2; 1] = [LOCAL_REAL_TOKEN_2022_RELEASE_V2];
#[cfg(not(feature = "laboratory-fixtures"))]
static COMPILED_COLLATERAL_RELEASES_V2: [AdapterReleaseV2; 0] = [];

const _: () = assert!(ADAPTER_RELEASE_V2_BYTES == 192);

/// Return the closed release catalog compiled into this program.
///
/// The local-real laboratory ELF has one binary-pinned Token-2022 row. Default
/// and public-cluster artifacts deliberately have no rows and therefore deny
/// collateral admission until a separately reviewed deployment manifest is
/// compiled into that exact ELF.
pub fn compiled_collateral_catalog_v2() -> Outcome<AdapterCatalogV2> {
    AdapterCatalogV2::new(&COMPILED_COLLATERAL_RELEASES_V2)
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))
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
