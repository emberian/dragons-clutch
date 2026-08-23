//! Closed Token-2022 Egg issuance release for local-real execution.
//!
//! Collateral and claims are deliberately separate release planes. A Realm may
//! select legacy SPL collateral while this program still creates, mints, and
//! burns Token-2022 Eggs. No collateral release field is reinterpreted as a
//! claim-program authority.

use clutch_collateral_adapter_v2::{
    bind_claim_issuance_v1, AdapterReleaseV2, BoundClaimIssuanceV1, BoundCollateralProfileV2,
    ClaimIssuanceBindingV1, ClaimRuntimeObservationV1, Id, CLAIM_FLAGS_V1, TOKEN_2022_PROGRAM,
};
use solana_account_info::AccountInfo;

use crate::accounts::{require, Outcome};
use crate::collateral_release::{
    authenticate_collateral_release_deployment_v2, LOCAL_REAL_TOKEN_2022_DEPLOYMENT_ID_V2,
    LOCAL_REAL_TOKEN_2022_RELEASE_V2,
};
use crate::error::{ClutchError, Refusal};

const AUTHENTICATED_CLAIM_RELEASE_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/claim-release/current-loader-receipt/v1\0";

/// Frozen local-real claim-adapter release identity.
pub const LOCAL_REAL_CLAIM_ADAPTER_RELEASE_ID_V1: Id = Id::from_bytes([
    0x86, 0x37, 0x78, 0x40, 0xa1, 0x57, 0xb1, 0x92, 0x3b, 0x10, 0xb0, 0xce, 0xa6, 0x72, 0xa2, 0xa7,
    0x65, 0xbe, 0x30, 0xb5, 0xc0, 0xc6, 0x7f, 0xf1, 0x70, 0xd2, 0x27, 0x64, 0xfb, 0xbd, 0xd9, 0x21,
]);

/// Frozen local-real claim parser/CPI component identity.
pub const LOCAL_REAL_CLAIM_PARSER_CPI_CODE_ID_V1: Id = Id::from_bytes([
    0x99, 0xf0, 0x89, 0xcd, 0xf2, 0xaf, 0x08, 0x6f, 0x84, 0x63, 0x17, 0x78, 0x2a, 0x88, 0x05, 0x7d,
    0x36, 0xb8, 0xe1, 0x56, 0x57, 0x92, 0xe1, 0x15, 0xae, 0x1d, 0x5a, 0xc9, 0xd2, 0x2a, 0x8b, 0x11,
]);

/// Exact independent Token-2022 Egg issuance binding in the local-real ELF.
pub const LOCAL_REAL_CLAIM_ISSUANCE_BINDING_V1: ClaimIssuanceBindingV1 = ClaimIssuanceBindingV1 {
    flags: CLAIM_FLAGS_V1,
    adapter_release: LOCAL_REAL_CLAIM_ADAPTER_RELEASE_ID_V1,
    token_program: TOKEN_2022_PROGRAM,
    token_program_deployment: LOCAL_REAL_TOKEN_2022_DEPLOYMENT_ID_V2,
    parser_cpi_code: LOCAL_REAL_CLAIM_PARSER_CPI_CODE_ID_V1,
    decimals: 0,
    mint_extensions: 0,
    account_extensions: 0,
};

/// Checked claim-plane row selected by this exact program build.
///
/// Claim issuance is independent of Realm collateral selection, but still
/// names one exact Token-2022 release from the same closed deployment catalog.
/// No instruction account or environment variable may supply either field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompiledClaimIssuanceReleaseV1 {
    binding: ClaimIssuanceBindingV1,
    token_release: AdapterReleaseV2,
}

impl CompiledClaimIssuanceReleaseV1 {
    pub(crate) const fn checked(
        binding: ClaimIssuanceBindingV1,
        token_release: AdapterReleaseV2,
    ) -> Self {
        Self {
            binding,
            token_release,
        }
    }
}

#[cfg(feature = "laboratory-fixtures")]
const COMPILED_CLAIM_ISSUANCE_RELEASE_V1: Option<CompiledClaimIssuanceReleaseV1> =
    Some(CompiledClaimIssuanceReleaseV1::checked(
        LOCAL_REAL_CLAIM_ISSUANCE_BINDING_V1,
        LOCAL_REAL_TOKEN_2022_RELEASE_V2,
    ));

#[cfg(all(
    not(feature = "laboratory-fixtures"),
    not(feature = "observed-positive-collateral-release-manifest")
))]
const COMPILED_CLAIM_ISSUANCE_RELEASE_V1: Option<CompiledClaimIssuanceReleaseV1> = None;

#[cfg(feature = "observed-positive-collateral-release-manifest")]
fn compiled_claim_issuance_release_v1() -> Option<CompiledClaimIssuanceReleaseV1> {
    crate::observed_collateral_release_manifest_v2::OBSERVED_CLAIM_ISSUANCE_RELEASE_V1
}

#[cfg(not(feature = "observed-positive-collateral-release-manifest"))]
fn compiled_claim_issuance_release_v1() -> Option<CompiledClaimIssuanceReleaseV1> {
    COMPILED_CLAIM_ISSUANCE_RELEASE_V1
}

fn validate_compiled_claim_issuance_release_v1(
    manifest: CompiledClaimIssuanceReleaseV1,
) -> Outcome<()> {
    manifest
        .binding
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    manifest
        .token_release
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    require(
        manifest.binding.token_program == manifest.token_release.token_program
            && manifest.binding.token_program_deployment
                == manifest.token_release.token_program_deployment
            && manifest.binding.parser_cpi_code != manifest.token_release.parser_cpi_code,
        ClutchError::AuthorizationUnavailable,
    )?;
    manifest
        .binding
        .require_separate_from_collateral(manifest.token_release)
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))
}

fn require_claim_release_separate_from_collateral_v1(
    claim: BoundClaimIssuanceV1,
    collateral_release: AdapterReleaseV2,
) -> Outcome<()> {
    claim
        .binding()
        .require_separate_from_collateral(collateral_release)
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))
}

/// Private runtime proof that the independently selected claim plane is the
/// exact current loader deployment named by its compiled release and is
/// distinct from the authenticated Realm collateral release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedClaimIssuanceReleaseV1 {
    bound: BoundClaimIssuanceV1,
    token_programdata: Id,
    deployment_slot: u64,
    loader_receipt_id: Id,
    receipt_id: Id,
}

impl AuthenticatedClaimIssuanceReleaseV1 {
    /// Pure claim authority consumed by the collateral/claim transition.
    pub(crate) const fn bound(self) -> BoundClaimIssuanceV1 {
        self.bound
    }

    /// Exact current ProgramData account observed at admission.
    pub(crate) const fn token_programdata(self) -> Id {
        self.token_programdata
    }

    /// Loader-recorded deployment slot retained for Product founding proof.
    pub(crate) const fn deployment_slot(self) -> u64 {
        self.deployment_slot
    }

    /// Exact current loader observation receipt.
    pub(crate) const fn loader_receipt_id(self) -> Id {
        self.loader_receipt_id
    }

    /// Combined collateral-separation and current claim-release receipt.
    pub(crate) const fn receipt_id(self) -> Id {
        self.receipt_id
    }
}

/// Withdrawn program-account-only admission retained so older routes fail
/// closed instead of manufacturing deployment evidence from a compiled row.
pub fn authenticate_claim_issuance_v1(
    collateral: BoundCollateralProfileV2,
    token_program: &AccountInfo<'_>,
) -> Outcome<BoundClaimIssuanceV1> {
    let _ = (collateral, token_program);
    Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
}

/// Authenticate the claim plane against exact current Token-2022
/// ProgramData before joining it to an independently authenticated collateral
/// release.
pub fn authenticate_claim_issuance_with_programdata_v1(
    collateral: BoundCollateralProfileV2,
    token_program: &AccountInfo<'_>,
    token_programdata: &AccountInfo<'_>,
) -> Outcome<BoundClaimIssuanceV1> {
    Ok(authenticate_claim_issuance_release_with_programdata_v1(
        collateral,
        token_program,
        token_programdata,
    )?
    .bound())
}

/// Mint a private Product-consumable release proof while returning no
/// caller-shaped deployment fields.
pub(crate) fn authenticate_claim_issuance_release_with_programdata_v1(
    collateral: BoundCollateralProfileV2,
    token_program: &AccountInfo<'_>,
    token_programdata: &AccountInfo<'_>,
) -> Outcome<AuthenticatedClaimIssuanceReleaseV1> {
    let (bound, deployment) = authenticate_claim_issuance_runtime_release_with_programdata_v1(
        token_program,
        token_programdata,
    )?;
    require_claim_release_separate_from_collateral_v1(bound, collateral.release())?;
    let collateral_release_id = collateral
        .release()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let receipt_id = Id::from_bytes(
        solana_sha256_hasher::hashv(&[
            AUTHENTICATED_CLAIM_RELEASE_RECEIPT_DOMAIN_V1,
            &collateral.market().market.bytes(),
            &collateral.realm_bound().realm().realm.bytes(),
            &collateral.policy_id().bytes(),
            &collateral_release_id.bytes(),
            &bound.binding_id().bytes(),
            &deployment.programdata_account().bytes(),
            &deployment.deployment_slot().to_le_bytes(),
            &deployment.receipt_id().bytes(),
        ])
        .to_bytes(),
    );
    receipt_id
        .require_live()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    Ok(AuthenticatedClaimIssuanceReleaseV1 {
        bound,
        token_programdata: deployment.programdata_account(),
        deployment_slot: deployment.deployment_slot(),
        loader_receipt_id: deployment.receipt_id(),
        receipt_id,
    })
}

/// Withdrawn program-account-only runtime admission. Current routes must use
/// [`authenticate_claim_issuance_runtime_with_programdata_v1`].
pub fn authenticate_claim_issuance_runtime_v1(
    token_program: &AccountInfo<'_>,
) -> Outcome<BoundClaimIssuanceV1> {
    let _ = token_program;
    Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
}

/// Authenticate the independently selected claim release against current
/// Upgradeable Loader state and exact deployed ELF bytes.
pub fn authenticate_claim_issuance_runtime_with_programdata_v1(
    token_program: &AccountInfo<'_>,
    token_programdata: &AccountInfo<'_>,
) -> Outcome<BoundClaimIssuanceV1> {
    Ok(authenticate_claim_issuance_runtime_release_with_programdata_v1(
        token_program,
        token_programdata,
    )?
    .0)
}

fn authenticate_claim_issuance_runtime_release_with_programdata_v1(
    token_program: &AccountInfo<'_>,
    token_programdata: &AccountInfo<'_>,
) -> Outcome<(
    BoundClaimIssuanceV1,
    crate::collateral_release::AuthenticatedCollateralReleaseDeploymentV2,
)> {
    let manifest = compiled_claim_issuance_release_v1()
        .ok_or(Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    validate_compiled_claim_issuance_release_v1(manifest)?;
    let deployment = authenticate_collateral_release_deployment_v2(
        manifest.token_release,
        token_program,
        token_programdata,
    )?;
    let expected = manifest
        .binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let bound = bind_claim_issuance_v1(
        expected,
        manifest.binding,
        ClaimRuntimeObservationV1 {
            token_program: Id::from_bytes(token_program.key.to_bytes()),
            token_program_executable: token_program.executable,
            token_program_writable: token_program.is_writable,
            token_program_signer: token_program.is_signer,
            token_program_deployment: deployment.release().token_program_deployment,
            parser_cpi_code: manifest.binding.parser_cpi_code,
        },
        manifest.token_release,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    Ok((bound, deployment))
}

#[cfg(test)]
mod checked_claim_release_tests {
    use super::*;

    #[test]
    fn checked_claim_manifest_refuses_deployment_and_parser_aliases() {
        let valid = CompiledClaimIssuanceReleaseV1::checked(
            LOCAL_REAL_CLAIM_ISSUANCE_BINDING_V1,
            LOCAL_REAL_TOKEN_2022_RELEASE_V2,
        );
        assert!(validate_compiled_claim_issuance_release_v1(valid).is_ok());

        let wrong_deployment = CompiledClaimIssuanceReleaseV1::checked(
            ClaimIssuanceBindingV1 {
                token_program_deployment: Id::from_bytes([93; 32]),
                ..LOCAL_REAL_CLAIM_ISSUANCE_BINDING_V1
            },
            LOCAL_REAL_TOKEN_2022_RELEASE_V2,
        );
        assert!(validate_compiled_claim_issuance_release_v1(wrong_deployment).is_err());

        let aliased_parser = CompiledClaimIssuanceReleaseV1::checked(
            ClaimIssuanceBindingV1 {
                parser_cpi_code: LOCAL_REAL_TOKEN_2022_RELEASE_V2.parser_cpi_code,
                ..LOCAL_REAL_CLAIM_ISSUANCE_BINDING_V1
            },
            LOCAL_REAL_TOKEN_2022_RELEASE_V2,
        );
        assert!(validate_compiled_claim_issuance_release_v1(aliased_parser).is_err());
    }

    #[test]
    fn selected_claim_binding_not_local_constant_owns_collateral_separation() {
        let selected_binding = ClaimIssuanceBindingV1 {
            adapter_release: Id::from_bytes([94; 32]),
            parser_cpi_code: Id::from_bytes([95; 32]),
            ..LOCAL_REAL_CLAIM_ISSUANCE_BINDING_V1
        };
        let expected = selected_binding.id().unwrap();
        let bound = bind_claim_issuance_v1(
            expected,
            selected_binding,
            ClaimRuntimeObservationV1 {
                token_program: selected_binding.token_program,
                token_program_executable: true,
                token_program_writable: false,
                token_program_signer: false,
                token_program_deployment: selected_binding.token_program_deployment,
                parser_cpi_code: selected_binding.parser_cpi_code,
            },
            LOCAL_REAL_TOKEN_2022_RELEASE_V2,
        )
        .unwrap();
        let aliased_collateral = AdapterReleaseV2::legacy_spl(
            Id::from_bytes([96; 32]),
            selected_binding.parser_cpi_code,
        );
        assert!(LOCAL_REAL_CLAIM_ISSUANCE_BINDING_V1
            .require_separate_from_collateral(aliased_collateral)
            .is_ok());
        assert!(require_claim_release_separate_from_collateral_v1(bound, aliased_collateral)
            .is_err());
    }
}
