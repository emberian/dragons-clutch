//! Resolution-owned terminal result authentication shared by Claims routes.
//!
//! Core persists the exact certificate account identity after authenticating
//! its Resolution PDA. Claims therefore rejoins that identity to the current
//! slot-pinned Resolution role and consumes the certificate's typed result;
//! it never synthesizes a second Core-owned terminal fact.

use dclutch_market_core_codec::CoreState;
use dclutch_product_payoff_v2_codec::runtime_v3::BasisKindV3;
use dclutch_rational_representation_v2_kernel::product_v3::TerminalScenarioV3;
use dclutch_release_set_contract::ExecutionRoleV1;
use dclutch_resolution_codec::{
    RESOLUTION_CERTIFICATE_BYTES_V2, ResolutionCertificateKindV2, ResolutionCertificateV2,
};
use solana_program::{
    account_info::AccountInfo, program_error::ProgramError, rent::Rent, sysvar::SysvarSerialize,
};

use super::ClaimsSbfError;

/// Exact accounts that make one live Resolution certificate authoritative.
#[derive(Clone, Copy)]
pub(crate) struct TerminalCertificateFrameV3<'accounts, 'info> {
    pub(crate) registry: &'accounts AccountInfo<'info>,
    pub(crate) cache: &'accounts AccountInfo<'info>,
    pub(crate) resolution_program: &'accounts AccountInfo<'info>,
    pub(crate) resolution_programdata: &'accounts AccountInfo<'info>,
    pub(crate) certificate: &'accounts AccountInfo<'info>,
    pub(crate) rent: &'accounts AccountInfo<'info>,
}

/// Authenticate Core's exact certificate and project one Claims terminal case.
#[inline(never)]
pub(crate) fn authenticate_terminal_certificate_scenario_v3(
    frame: TerminalCertificateFrameV3<'_, '_>,
    release_set: [u8; 32],
    core: CoreState,
    basis_kind: BasisKindV3,
    outcome_count: u32,
) -> Result<TerminalScenarioV3, ProgramError> {
    crate::authenticate_activated_role(
        frame.registry,
        frame.cache,
        ExecutionRoleV1::Resolution,
        frame.resolution_program,
        frame.resolution_programdata,
        &release_set,
    )?;
    let expected_certificate = core
        .terminal_receipt
        .ok_or(ClaimsSbfError::Identity)?
        .to_bytes();
    let rent = Rent::from_account_info(frame.rent).map_err(|_| ClaimsSbfError::Accounts)?;
    if frame.certificate.key.to_bytes() != expected_certificate
        || frame.certificate.owner != frame.resolution_program.key
        || frame.certificate.data_len() != RESOLUTION_CERTIFICATE_BYTES_V2
        || frame.certificate.is_signer
        || frame.certificate.is_writable
        || frame.certificate.executable
        || !rent.is_exempt(
            frame.certificate.lamports(),
            RESOLUTION_CERTIFICATE_BYTES_V2,
        )
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    let bytes = frame
        .certificate
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let certificate =
        ResolutionCertificateV2::decode(&bytes).map_err(|_| ClaimsSbfError::Identity)?;
    if certificate.receipt_account != expected_certificate
        || certificate.market != core.identity.market_id.to_bytes()
        || certificate.source_material != core.identity.resolution_policy.to_bytes()
        || certificate.product_record_digest != core.identity.product_record.to_bytes()
        || certificate.generation != core.identity.generation
        || certificate.selector != core.terminal_winner
        || core.terminal_winner >= outcome_count
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    certificate
        .validate_terminal_product(core.identity.product_record.to_bytes(), outcome_count)
        .map_err(|_| ClaimsSbfError::Identity)?;
    match (basis_kind, certificate.kind) {
        (
            BasisKindV3::CategoricalQ1,
            ResolutionCertificateKindV2::ResolutionSuccess
            | ResolutionCertificateKindV2::ResolutionFailure,
        ) => Ok(TerminalScenarioV3::Categorical(core.terminal_winner)),
        // **The spline family settles exactly as the graded family does**, and
        // that is a result rather than a convenience. `TerminalScenarioV3` has
        // three variants -- categorical selection, a rational coordinate, and
        // failure -- and the design flagged that a curved basis might need a
        // fourth, which would have cascaded into the settlement evaluator's
        // wildcard and the wallet driver.
        //
        // It does not. A spline consumes the same rational coordinate the
        // graded family does and returns the same width-sized partition, and it
        // carries an explicit failure payout vector in the same tail slot. So
        // the two families share these arms outright, which is the strongest
        // available statement that no new terminal shape was invented: the
        // pairs are widened, not added to.
        (
            BasisKindV3::GradedExactComplement | BasisKindV3::SplineDegree2To3 { .. },
            ResolutionCertificateKindV2::ResolutionSuccess,
        ) => Ok(TerminalScenarioV3::Rational {
            numerator: certificate.result_numerator,
            denominator: certificate.result_denominator,
        }),
        (
            BasisKindV3::GradedExactComplement | BasisKindV3::SplineDegree2To3 { .. },
            ResolutionCertificateKindV2::ResolutionFailure,
        ) => Ok(TerminalScenarioV3::Failure),
        (
            BasisKindV3::CategoricalQ1
            | BasisKindV3::GradedExactComplement
            | BasisKindV3::SplineDegree2To3 { .. },
            ResolutionCertificateKindV2::RecoveryAdvanced | ResolutionCertificateKindV2::Exhausted,
        ) => Err(ClaimsSbfError::Identity.into()),
    }
}
