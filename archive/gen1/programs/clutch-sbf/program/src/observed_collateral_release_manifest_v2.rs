//! Checked observed-positive collateral releases for a deployable profile.
//!
//! This file, not an environment variable or instruction account, is the
//! release-manifest boundary. A reviewed public/devnet build adds one const
//! `AdapterReleaseV2` per exact token-program ELF and the matching
//! `CompiledCollateralReleaseManifestV2` row containing the linked ProgramData
//! account, positive deployment slot, and authority state observed at the
//! release checkpoint. The two arrays must remain ordered and equal length.
//! Token-2022 outcome issuance additionally selects one independently reviewed
//! `CompiledClaimIssuanceReleaseV1`; it is never inferred from Realm collateral.
//!
//! No public-cluster observation has been ratified in this repository yet, so
//! the checked manifest is empty and the feature remains fail-closed.

use clutch_collateral_adapter_v2::AdapterReleaseV2;

use super::collateral_release::{
    CollateralUpgradeAuthorityV2, CompiledCollateralReleaseManifestV2,
};
use super::claim_release::CompiledClaimIssuanceReleaseV1;

// A reviewed row has this checked-source shape (never populate it from env):
//
// const RELEASE: AdapterReleaseV2 = AdapterReleaseV2::token_2022_base(
//     Id::from_bytes(EXACT_ELF_SHA256),
//     Id::from_bytes(EXACT_PARSER_CPI_MANIFEST_SHA256),
// );
// const MANIFEST: CompiledCollateralReleaseManifestV2 =
//     CompiledCollateralReleaseManifestV2::observed_positive(
//         RELEASE,
//         Id::from_bytes(EXACT_LINKED_PROGRAMDATA),
//         EXACT_POSITIVE_DEPLOYMENT_SLOT,
//         CollateralUpgradeAuthorityV2::Present(Id::from_bytes(EXACT_AUTHORITY)),
//     );
// Immutable ProgramData uses `CollateralUpgradeAuthorityV2::Immutable`.

const _: Option<CollateralUpgradeAuthorityV2> = None;

pub(super) static OBSERVED_COLLATERAL_RELEASES_V2: [AdapterReleaseV2; 0] = [];
pub(super) static OBSERVED_COLLATERAL_RELEASE_MANIFESTS_V2:
    [CompiledCollateralReleaseManifestV2; 0] = [];

// Outcome issuance is an independent Token-2022 release plane. A reviewed
// public/devnet build must populate this separately even when one Realm also
// selects Token-2022 collateral; absence keeps mint/burn routes disabled.
pub(super) const OBSERVED_CLAIM_ISSUANCE_RELEASE_V1:
    Option<CompiledClaimIssuanceReleaseV1> = None;

const _: () = assert!(
    OBSERVED_COLLATERAL_RELEASES_V2.len()
        == OBSERVED_COLLATERAL_RELEASE_MANIFESTS_V2.len()
);
