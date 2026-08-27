//! Chain-derived Product V3 graded-basis Registry records.
//!
//! This host-only adapter derives raw/staging PDAs from exact bytes and schema
//! IDs. It independently reruns the categorical projection before publishing
//! certificate or admission bytes. It never signs, uploads, finalizes, or
//! submits a transaction.

use dclutch_product_compiler::noncategorical_v3::{
    CategoricalProjectionBoundaryV3, certify_categorical_approximation_v3,
    recheck_categorical_approximation_v3,
};
use dclutch_product_payoff_v2_codec::registry_v3::{
    APPROXIMATION_CERTIFICATE_BYTES_V3, GRADED_BASIS_ADMISSION_BYTES_V3,
    GRADED_BASIS_ADMISSION_SCHEMA_ID_V3, GRADED_BASIS_CERTIFICATE_SCHEMA_ID_V3,
    GRADED_BASIS_RECORD_SCHEMA_ID_V3, GradedBasisAdmissionV3, admit_authenticated_graded_basis_v3,
    derive_graded_basis_admission_v3, raw_record_digest_v3,
};
use dclutch_product_runtime_v2::{ContentId, ResultDomainV2};
use dclutch_product_runtime_v2_admission::{
    FinalizedRecordCoordinateV2, RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use solana_program::pubkey::Pubkey;

use crate::{AccountObservationV2, Error, FinalizedRecordObservationV2, Result, validate_record};

/// Exact nonzero compiler evidence selected for one finalized admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GradedBasisCompilerEvidenceV3 {
    /// Exact compiler semantic release or ArtifactRelease content identity.
    pub compiler_release_id: ContentId,
    /// Exact reproducible compiler toolchain identity.
    pub toolchain_id: ContentId,
}

/// Chain-derived coordinates and immutable bodies for one V3 admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledGradedBasisAdmissionV3 {
    /// Already-finalized Product Runtime V2 ResultDomain coordinate.
    pub result_domain: FinalizedRecordCoordinateV2,
    /// Runtime graded-basis raw/staging coordinate.
    pub linked_basis: FinalizedRecordCoordinateV2,
    /// Exact 256-byte certificate raw/staging coordinate.
    pub certificate: FinalizedRecordCoordinateV2,
    /// Exact 304-byte admission raw/staging coordinate.
    pub admission: FinalizedRecordCoordinateV2,
    /// Decoded admission binding written to the admission raw record.
    pub binding: GradedBasisAdmissionV3,
}

/// Compile, independently recheck, and derive all Registry record coordinates.
///
/// Outputs change only after basis/domain/certificate recomputation and every
/// admission join succeeds.
pub fn compile_graded_basis_admission_v3(
    registry_program: Pubkey,
    result_domain_bytes: &[u8],
    linked_basis_bytes: &[u8],
    boundary: CategoricalProjectionBoundaryV3,
    compiler: GradedBasisCompilerEvidenceV3,
    certificate_output: &mut [u8],
    admission_output: &mut [u8],
) -> Result<CompiledGradedBasisAdmissionV3> {
    if certificate_output.len() != APPROXIMATION_CERTIFICATE_BYTES_V3
        || admission_output.len() != GRADED_BASIS_ADMISSION_BYTES_V3
    {
        return Err(Error::OutputLength);
    }
    let domain = ResultDomainV2::decode(result_domain_bytes).map_err(|_| Error::RuntimeProduct)?;
    let approximation =
        certify_categorical_approximation_v3(result_domain_bytes, linked_basis_bytes, boundary)
            .map_err(|_| Error::Admission)?;
    let certificate_bytes = approximation.certificate.to_bytes();
    recheck_categorical_approximation_v3(
        result_domain_bytes,
        linked_basis_bytes,
        &certificate_bytes,
        &approximation.payouts,
        &approximation.component_error_bounds,
    )
    .map_err(|_| Error::Admission)?;
    let binding = derive_graded_basis_admission_v3(
        result_domain_bytes,
        linked_basis_bytes,
        &certificate_bytes,
        compiler.compiler_release_id.to_bytes(),
        compiler.toolchain_id.to_bytes(),
    )
    .map_err(|_| Error::Admission)?;
    let admission_bytes = binding.to_bytes();
    admit_authenticated_graded_basis_v3(
        result_domain_bytes,
        linked_basis_bytes,
        &certificate_bytes,
        &admission_bytes,
    )
    .map_err(|_| Error::Admission)?;
    let result_domain_digest = raw_record_digest_v3(result_domain_bytes);
    if result_domain_digest != binding.result_domain_id()
        || domain.product_id().to_bytes() != binding.product_id()
    {
        return Err(Error::CrossRecordMismatch);
    }
    let linked_basis_digest = raw_record_digest_v3(linked_basis_bytes);
    let certificate_digest = raw_record_digest_v3(&certificate_bytes);
    let admission_digest = raw_record_digest_v3(&admission_bytes);
    let result = CompiledGradedBasisAdmissionV3 {
        result_domain: coordinate(
            registry_program,
            RESULT_DOMAIN_SCHEMA_ID_V2,
            result_domain_digest,
        )?,
        linked_basis: coordinate(
            registry_program,
            GRADED_BASIS_RECORD_SCHEMA_ID_V3,
            linked_basis_digest,
        )?,
        certificate: coordinate(
            registry_program,
            GRADED_BASIS_CERTIFICATE_SCHEMA_ID_V3,
            certificate_digest,
        )?,
        admission: coordinate(
            registry_program,
            GRADED_BASIS_ADMISSION_SCHEMA_ID_V3,
            admission_digest,
        )?,
        binding,
    };
    certificate_output.copy_from_slice(&certificate_bytes);
    admission_output.copy_from_slice(&admission_bytes);
    Ok(result)
}

/// Same-finalized snapshot of all four Registry raw/staging pairs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalizedGradedBasisStateV3<'a> {
    /// Executable Registry program observation.
    pub registry: AccountObservationV2<'a>,
    /// Product Runtime V2 ResultDomain record.
    pub result_domain: FinalizedRecordObservationV2<'a>,
    /// Product V3 linked basis record.
    pub linked_basis: FinalizedRecordObservationV2<'a>,
    /// Product V3 projection/error certificate record.
    pub certificate: FinalizedRecordObservationV2<'a>,
    /// Product V3 admission binding record.
    pub admission: FinalizedRecordObservationV2<'a>,
}

/// Reacquire one same-slot finalized record graph and rerun its semantic join.
pub fn validate_finalized_graded_basis_admission_v3(
    compiled: CompiledGradedBasisAdmissionV3,
    state: FinalizedGradedBasisStateV3<'_>,
) -> Result<GradedBasisAdmissionV3> {
    let slot = state.registry.slot;
    let observations = [
        state.result_domain.raw,
        state.result_domain.staging,
        state.linked_basis.raw,
        state.linked_basis.staging,
        state.certificate.raw,
        state.certificate.staging,
        state.admission.raw,
        state.admission.staging,
    ];
    if !state.registry.executable
        || observations
            .iter()
            .any(|observation| observation.slot != slot)
    {
        return Err(Error::ObservationMismatch);
    }
    for (coordinate, observation) in [
        (compiled.result_domain, state.result_domain),
        (compiled.linked_basis, state.linked_basis),
        (compiled.certificate, state.certificate),
        (compiled.admission, state.admission),
    ] {
        validate_record(state.registry.key, coordinate, observation)?;
    }
    let binding = admit_authenticated_graded_basis_v3(
        state.result_domain.raw.data,
        state.linked_basis.raw.data,
        state.certificate.raw.data,
        state.admission.raw.data,
    )
    .map_err(|_| Error::Admission)?;
    if binding != compiled.binding {
        return Err(Error::CrossRecordMismatch);
    }
    Ok(binding)
}

fn coordinate(
    registry_program: Pubkey,
    schema: [u8; 32],
    digest: [u8; 32],
) -> Result<FinalizedRecordCoordinateV2> {
    let content_digest = ContentId::new(digest).map_err(|_| Error::Admission)?;
    let (raw, _) = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
        &registry_program,
    );
    let (staging, _) = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
        &registry_program,
    );
    Ok(FinalizedRecordCoordinateV2 {
        schema_id: ContentId::new(schema).map_err(|_| Error::Admission)?,
        content_digest,
        raw_account: ContentId::new(raw.to_bytes()).map_err(|_| Error::Admission)?,
        staging_account: ContentId::new(staging.to_bytes()).map_err(|_| Error::Admission)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_product_payoff_v2_codec::registry_v3::{
        GRADED_BASIS_ADMISSION_CERTIFICATE_DIGEST_OFFSET_V3,
        GRADED_BASIS_ADMISSION_COMPILER_RELEASE_OFFSET_V3,
        GRADED_BASIS_ADMISSION_TOOLCHAIN_OFFSET_V3,
    };
    use dclutch_product_payoff_v2_codec::runtime_v3::{
        BasisInputV3, BasisKindV3, BasisShapeV3, BasisTermV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        basis_record_bytes_v3, compile_basis_v3, semantic_basis_preimage_v3,
    };
    use dclutch_product_runtime_v2::{
        ResultDomainInputV2, compile_result_domain_v2, result_domain_record_bytes,
    };
    use solana_program::hash::hashv;
    use solana_sdk_ids::system_program;

    fn id(fill: u8) -> [u8; 32] {
        [fill; 32]
    }

    fn content(fill: u8) -> ContentId {
        ContentId::new(id(fill)).expect("identity")
    }

    struct Fixture {
        domain: Vec<u8>,
        basis: Vec<u8>,
    }

    fn fixture() -> Fixture {
        let product_id = id(1);
        let coordinate_domain_id = id(2);
        let result_unit_id = id(3);
        let evaluator_release_id = id(4);
        let knots = [0, 10, 20];
        let terms = [
            BasisTermV3 {
                claim_index: 0,
                shape: BasisShapeV3::RampUp { left: 0, right: 2 },
                amplitude: 60,
            },
            BasisTermV3 {
                claim_index: 1,
                shape: BasisShapeV3::Tent {
                    left: 0,
                    peak: 1,
                    right: 2,
                },
                amplitude: 30,
            },
        ];
        let input = BasisInputV3 {
            kind: BasisKindV3::GradedExactComplement,
            product_id,
            result_domain_id: id(9),
            coordinate_domain_id,
            result_unit_id,
            evaluator_release_id,
            basis_width: 3,
            payout_scale: 100,
            knot_denominator: 1,
            knots: &knots,
            terms: &terms,
            failure_payouts: &[1, 2, 97],
        };
        let basis_bytes =
            basis_record_bytes_v3(input.kind, 3, knots.len(), terms.len()).expect("basis bytes");
        let mut provisional = vec![0_u8; basis_bytes];
        compile_basis_v3(input, &mut provisional).expect("provisional basis");
        let semantic = semantic_basis_preimage_v3(&provisional).expect("semantic basis");
        let semantic_basis_id = hashv(&[
            SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
            semantic.prefix(),
            semantic.suffix(),
        ])
        .to_bytes();
        let cuts = [-10, 0, 10, 20, 30];
        let mut domain = vec![0_u8; result_domain_record_bytes(cuts.len()).expect("domain bytes")];
        compile_result_domain_v2(
            ResultDomainInputV2 {
                product_id: content(1),
                coordinate_domain_id: content(2),
                result_unit_id: content(3),
                liability_basis_id: ContentId::new(semantic_basis_id).expect("basis identity"),
                representation_release_id: content(6),
                mapping_release_id: content(7),
                cut_denominator: 1,
                cuts: &cuts,
            },
            &mut domain,
        )
        .expect("domain");
        let mut basis = vec![0_u8; basis_bytes];
        compile_basis_v3(
            BasisInputV3 {
                result_domain_id: raw_record_digest_v3(&domain),
                ..input
            },
            &mut basis,
        )
        .expect("linked basis");
        Fixture { domain, basis }
    }

    fn compile(
        registry: Pubkey,
        fixture: &Fixture,
    ) -> (
        CompiledGradedBasisAdmissionV3,
        [u8; APPROXIMATION_CERTIFICATE_BYTES_V3],
        [u8; GRADED_BASIS_ADMISSION_BYTES_V3],
    ) {
        let mut certificate = [0_u8; APPROXIMATION_CERTIFICATE_BYTES_V3];
        let mut admission = [0_u8; GRADED_BASIS_ADMISSION_BYTES_V3];
        let compiled = compile_graded_basis_admission_v3(
            registry,
            &fixture.domain,
            &fixture.basis,
            CategoricalProjectionBoundaryV3::LeftCutClampedTails,
            GradedBasisCompilerEvidenceV3 {
                compiler_release_id: content(8),
                toolchain_id: content(9),
            },
            &mut certificate,
            &mut admission,
        )
        .expect("compiled Registry graph");
        (compiled, certificate, admission)
    }

    fn record<'a>(
        registry: Pubkey,
        coordinate: FinalizedRecordCoordinateV2,
        bytes: &'a [u8],
    ) -> FinalizedRecordObservationV2<'a> {
        FinalizedRecordObservationV2 {
            raw: AccountObservationV2 {
                slot: 41,
                key: Pubkey::new_from_array(coordinate.raw_account.to_bytes()),
                owner: registry,
                lamports: 10_000,
                executable: false,
                data: bytes,
            },
            staging: AccountObservationV2 {
                slot: 41,
                key: Pubkey::new_from_array(coordinate.staging_account.to_bytes()),
                owner: system_program::ID,
                lamports: 7,
                executable: false,
                data: &[],
            },
            raw_rent_minimum: 1,
        }
    }

    #[test]
    fn derives_exact_registry_graph_and_reacquires_same_slot() {
        let registry = Pubkey::new_unique();
        let fixture = fixture();
        let (compiled, certificate, admission) = compile(registry, &fixture);
        assert_eq!(
            compiled.linked_basis.schema_id.to_bytes(),
            GRADED_BASIS_RECORD_SCHEMA_ID_V3
        );
        assert_eq!(
            compiled.certificate.schema_id.to_bytes(),
            GRADED_BASIS_CERTIFICATE_SCHEMA_ID_V3
        );
        assert_eq!(
            compiled.admission.schema_id.to_bytes(),
            GRADED_BASIS_ADMISSION_SCHEMA_ID_V3
        );
        assert_eq!(
            compiled.binding.certificate_digest(),
            raw_record_digest_v3(&certificate)
        );
        let state = FinalizedGradedBasisStateV3 {
            registry: AccountObservationV2 {
                slot: 41,
                key: registry,
                owner: Pubkey::new_unique(),
                lamports: 1,
                executable: true,
                data: &[],
            },
            result_domain: record(registry, compiled.result_domain, &fixture.domain),
            linked_basis: record(registry, compiled.linked_basis, &fixture.basis),
            certificate: record(registry, compiled.certificate, &certificate),
            admission: record(registry, compiled.admission, &admission),
        };
        assert_eq!(
            validate_finalized_graded_basis_admission_v3(compiled, state),
            Ok(compiled.binding)
        );
        let mut foreign_owner = state;
        foreign_owner.linked_basis.raw.owner = Pubkey::new_unique();
        assert_eq!(
            validate_finalized_graded_basis_admission_v3(compiled, foreign_owner),
            Err(Error::RecordMismatch)
        );
        let mut live_staging = state;
        live_staging.certificate.staging.data = &[1];
        assert_eq!(
            validate_finalized_graded_basis_admission_v3(compiled, live_staging),
            Err(Error::RecordMismatch)
        );
        let mut stale = state;
        stale.admission.raw.slot = 40;
        assert_eq!(
            validate_finalized_graded_basis_admission_v3(compiled, stale),
            Err(Error::ObservationMismatch)
        );
    }

    #[test]
    fn all_authority_substitutions_refuse_and_outputs_are_atomic() {
        let registry = Pubkey::new_unique();
        let fixture = fixture();
        let (compiled, certificate, admission) = compile(registry, &fixture);
        for offset in [32_usize, 96, 128] {
            let mut hostile = fixture.basis.clone();
            *hostile.get_mut(offset).expect("basis offset") ^= 1;
            assert!(
                admit_authenticated_graded_basis_v3(
                    &fixture.domain,
                    &hostile,
                    &certificate,
                    &admission,
                )
                .is_err()
            );
        }
        let mut hostile_rounding = fixture.basis.clone();
        *hostile_rounding.get_mut(17).expect("rounding") = 0;
        assert!(
            admit_authenticated_graded_basis_v3(
                &fixture.domain,
                &hostile_rounding,
                &certificate,
                &admission,
            )
            .is_err()
        );
        let mut domain = fixture.domain.clone();
        *domain.get_mut(64).expect("coordinate domain") ^= 1;
        assert!(
            admit_authenticated_graded_basis_v3(&domain, &fixture.basis, &certificate, &admission,)
                .is_err()
        );
        {
            let offset = GRADED_BASIS_ADMISSION_CERTIFICATE_DIGEST_OFFSET_V3;
            let mut hostile = admission;
            *hostile.get_mut(offset).expect("certificate digest") ^= 1;
            assert!(
                admit_authenticated_graded_basis_v3(
                    &fixture.domain,
                    &fixture.basis,
                    &certificate,
                    &hostile,
                )
                .is_err()
            );
        }
        for offset in [
            GRADED_BASIS_ADMISSION_COMPILER_RELEASE_OFFSET_V3,
            GRADED_BASIS_ADMISSION_TOOLCHAIN_OFFSET_V3,
        ] {
            let mut hostile = admission;
            *hostile.get_mut(offset).expect("compiler identity") ^= 1;
            assert_ne!(
                raw_record_digest_v3(&hostile),
                compiled.admission.content_digest.to_bytes()
            );
            let state = FinalizedGradedBasisStateV3 {
                registry: AccountObservationV2 {
                    slot: 41,
                    key: registry,
                    owner: Pubkey::new_unique(),
                    lamports: 1,
                    executable: true,
                    data: &[],
                },
                result_domain: record(registry, compiled.result_domain, &fixture.domain),
                linked_basis: record(registry, compiled.linked_basis, &fixture.basis),
                certificate: record(registry, compiled.certificate, &certificate),
                admission: record(registry, compiled.admission, &hostile),
            };
            assert_eq!(
                validate_finalized_graded_basis_admission_v3(compiled, state),
                Err(Error::RecordMismatch)
            );
        }
        let mut hostile_certificate = certificate;
        hostile_certificate[200] ^= 1;
        assert!(
            admit_authenticated_graded_basis_v3(
                &fixture.domain,
                &fixture.basis,
                &hostile_certificate,
                &admission,
            )
            .is_err()
        );
        let mut short_certificate = [0xa5_u8; APPROXIMATION_CERTIFICATE_BYTES_V3];
        let mut short_admission = [0x5a_u8; GRADED_BASIS_ADMISSION_BYTES_V3];
        assert_eq!(
            compile_graded_basis_admission_v3(
                registry,
                &fixture.domain,
                &fixture.basis,
                CategoricalProjectionBoundaryV3::LeftCutClampedTails,
                GradedBasisCompilerEvidenceV3 {
                    compiler_release_id: content(8),
                    toolchain_id: content(9),
                },
                &mut short_certificate[..255],
                &mut short_admission,
            ),
            Err(Error::OutputLength)
        );
        assert_eq!(
            short_certificate,
            [0xa5; APPROXIMATION_CERTIFICATE_BYTES_V3]
        );
        assert_eq!(short_admission, [0x5a; GRADED_BASIS_ADMISSION_BYTES_V3]);
    }
}
