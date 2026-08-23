//! Closed Token-2022 Egg issuance release for local-real execution.
//!
//! Collateral and claims are deliberately separate release planes. A Realm may
//! select legacy SPL collateral while this program still creates, mints, and
//! burns Token-2022 Eggs. No collateral release field is reinterpreted as a
//! claim-program authority.

use clutch_collateral_adapter_v2::{
    bind_claim_issuance_v1, BoundClaimIssuanceV1, BoundCollateralProfileV2, ClaimIssuanceBindingV1,
    ClaimRuntimeObservationV1, Id, CLAIM_FLAGS_V1, TOKEN_2022_PROGRAM,
};
use solana_account_info::AccountInfo;

use crate::accounts::Outcome;
use crate::collateral_release::{
    authenticate_immutable_collateral_release_v2, LOCAL_REAL_LEGACY_SPL_RELEASE_V2,
    LOCAL_REAL_TOKEN_2022_DEPLOYMENT_ID_V2, LOCAL_REAL_TOKEN_2022_RELEASE_V2,
};
use crate::error::{ClutchError, Refusal};

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

/// Withdrawn program-account-only admission retained so older routes fail
/// closed instead of manufacturing deployment evidence from a compiled row.
pub fn authenticate_claim_issuance_v1(
    collateral: BoundCollateralProfileV2,
    token_program: &AccountInfo<'_>,
) -> Outcome<BoundClaimIssuanceV1> {
    let _ = (collateral, token_program);
    Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
}

/// Authenticate the claim plane against exact immutable Token-2022
/// ProgramData before joining it to an independently authenticated collateral
/// release.
pub fn authenticate_claim_issuance_with_programdata_v1(
    collateral: BoundCollateralProfileV2,
    token_program: &AccountInfo<'_>,
    token_programdata: &AccountInfo<'_>,
) -> Outcome<BoundClaimIssuanceV1> {
    let bound = authenticate_claim_issuance_runtime_with_programdata_v1(
        token_program,
        token_programdata,
    )?;
    LOCAL_REAL_CLAIM_ISSUANCE_BINDING_V1
        .require_separate_from_collateral(collateral.release())
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    Ok(bound)
}

/// Withdrawn program-account-only runtime admission. Current routes must use
/// [`authenticate_claim_issuance_runtime_with_programdata_v1`].
pub fn authenticate_claim_issuance_runtime_v1(
    token_program: &AccountInfo<'_>,
) -> Outcome<BoundClaimIssuanceV1> {
    let _ = token_program;
    Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
}

/// Authenticate the independently selected claim release against immutable
/// Upgradeable Loader state and exact deployed ELF bytes.
pub fn authenticate_claim_issuance_runtime_with_programdata_v1(
    token_program: &AccountInfo<'_>,
    token_programdata: &AccountInfo<'_>,
) -> Outcome<BoundClaimIssuanceV1> {
    #[cfg(not(feature = "laboratory-fixtures"))]
    {
        let _ = (token_program, token_programdata);
        return Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable));
    }
    #[cfg(feature = "laboratory-fixtures")]
    {
        let deployment = authenticate_immutable_collateral_release_v2(
            LOCAL_REAL_TOKEN_2022_RELEASE_V2,
            token_program,
            token_programdata,
        )?;
        let binding = LOCAL_REAL_CLAIM_ISSUANCE_BINDING_V1;
        let expected = binding
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
        binding
            .require_separate_from_collateral(LOCAL_REAL_LEGACY_SPL_RELEASE_V2)
            .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
        bind_claim_issuance_v1(
            expected,
            binding,
            ClaimRuntimeObservationV1 {
                token_program: Id::from_bytes(token_program.key.to_bytes()),
                token_program_executable: token_program.executable,
                token_program_writable: token_program.is_writable,
                token_program_signer: token_program.is_signer,
                token_program_deployment: deployment.release().token_program_deployment,
                parser_cpi_code: LOCAL_REAL_CLAIM_PARSER_CPI_CODE_ID_V1,
            },
            LOCAL_REAL_TOKEN_2022_RELEASE_V2,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}
