//! Exact admitted-AOT evidence and caller-authority derivation.
//!
//! The eight strategy extras are not fixture choices. Their record addresses
//! come from the Registry schemas and their content joins through the selected
//! descriptor, strategy, certificate, admission and artifact release. Caller
//! authorities similarly come from the exact accelerator request bytes, one
//! per accelerator invocation -- which is one per candidate-bank chunk under
//! the chunked transport, and exactly one under the output-page transport.

use dclutch_capability_program_contract::v4::CapabilityProgramV4;
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::{
    encode_register_bank_into,
    v2::{
        AcceleratorOutputPageRequestV3, AcceleratorRequestV2, AcceleratorTransportProfileV2,
        AdmittedAcceleratorRequestV2, AuthenticatedInterpreterArtifactsV2, BankTransportV2,
        EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
        ExecutionStrategyAdmissionV2, ExecutionStrategyCertificateV2, ExecutionStrategyProgramV2,
        RequestTransportV2, accelerator_invocation_count_v2, classify_bank_transport_v2,
        validate_admitted_aot_v4,
    },
};
use dclutch_registry_contract::{
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1, DeploymentObservationV1,
    require_slot_pinned_release_v1,
};
use dclutch_registry_svm::{ProgramDataV3View, ProgramV3View};
use dclutch_release_set_contract::{ArtifactReleaseIdV1, CallerAuthoritySeedsV1, ExecutionRoleV1};
use sha2::{Digest, Sha256};
use solana_program::{hash::hash, pubkey::Pubkey};
use solana_sdk_ids::bpf_loader_upgradeable;

use crate::{
    BuilderError,
    artifacts::{ArtifactSetV1, DerivedRecordV1, derive_record, digest},
    frame::BuiltAccountV1,
};

/// Possibly absent chain observations needed by the admitted-AOT lane.
///
/// Absence is represented deliberately so snapshot importers can report a
/// stable refusal instead of inventing a record or deployment placeholder.
#[derive(Clone, Copy, Debug)]
pub struct AdmittedAotInputV1<'a> {
    /// Strategy-selected translation certificate bytes.
    pub certificate: Option<&'a [u8]>,
    /// Registry admission of that exact certificate.
    pub admission: Option<&'a [u8]>,
    /// Certificate-selected immutable artifact release bytes.
    pub artifact_release: Option<&'a [u8]>,
    /// Current Loader Program account, including its chain-true observation.
    pub accelerator_program: Option<&'a BuiltAccountV1>,
    /// Current Loader ProgramData account, including the complete ELF tail.
    pub accelerator_programdata: Option<&'a BuiltAccountV1>,
}

/// The authenticated eight-account evidence suffix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedAdmittedEvidenceV1 {
    /// Finalized certificate record.
    pub certificate: DerivedRecordV1,
    /// Finalized admission record.
    pub admission: DerivedRecordV1,
    /// Finalized artifact-release record.
    pub artifact_release: DerivedRecordV1,
    /// Exact current accelerator Program observation.
    pub accelerator_program: BuiltAccountV1,
    /// Exact current accelerator ProgramData observation.
    pub accelerator_programdata: BuiltAccountV1,
}

/// Inputs whose sole output is the canonical accelerator request sequence and
/// its Trading caller authorities.
pub struct AdmittedAuthorityInputV1<'a> {
    /// Current Trading program, which owns every caller-authority PDA.
    pub trading_program: Pubkey,
    /// Current release set.
    pub release_set: ContentId,
    /// Logical Core Market.
    pub market: Pubkey,
    /// Mutable family root.
    pub root: Pubkey,
    /// Finalized selected strategy identity.
    pub strategy_program: ContentId,
    /// Finalized selected certificate identity.
    pub certificate_program: ContentId,
    /// Finalized selected capability descriptor identity.
    pub capability_program: ContentId,
    /// Complete authenticated invocation-context digest.
    pub invocation_context: ContentId,
    /// Artifact-derived input transport. Output transport is independent of it.
    pub transport: RequestTransportV2,
    /// Strategy-selected output transport.
    pub profile: AcceleratorTransportProfileV2,
    /// Immutable admitted accelerator program, which owns any output page.
    pub accelerator_program: Pubkey,
    /// Product-authoritative runtime tail count.
    pub tail_count: u32,
    /// Exact pre-transition scalar bank.
    pub scalars: &'a [u64],
    /// Exact pre-transition identity bank.
    pub identities: &'a [[u8; 32]],
}

/// One exact request and the PDA Trading signs for its accelerator CPI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedAdmittedAuthorityV1 {
    /// Canonical chunk ordinal.
    pub chunk_index: u32,
    /// Exact encoded accelerator request bytes.
    pub request: Vec<u8>,
    /// SHA-256 of `request`.
    pub request_digest: [u8; 32],
    /// Release-pinned Trading caller authority.
    pub authority: Pubkey,
}

/// Complete pre-transition bank and its canonical request/authority sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedAdmittedAuthoritiesV1 {
    /// Scalar-then-identity register-bank bytes.
    pub input_bank: Vec<u8>,
    /// Inline or authenticated-scratch transport selected from bank width.
    pub transport: RequestTransportV2,
    /// Accelerator-owned output page, under `OutputPageV3` only.
    pub output_page: Option<Pubkey>,
    /// One entry per accelerator invocation this bank costs.
    pub entries: Vec<DerivedAdmittedAuthorityV1>,
}

fn content(bytes: [u8; 32]) -> Result<ContentId, BuilderError> {
    ContentId::new(bytes).map_err(|_| BuilderError::Artifact)
}

/// Authenticate the complete record/deployment chain and derive its accounts.
pub fn derive_admitted_evidence_v1(
    registry_program: Pubkey,
    set: ArtifactSetV1<'_>,
    input: AdmittedAotInputV1<'_>,
) -> Result<DerivedAdmittedEvidenceV1, BuilderError> {
    let certificate_bytes = input.certificate.ok_or(BuilderError::Artifact)?;
    let admission_bytes = input.admission.ok_or(BuilderError::Artifact)?;
    let artifact_bytes = input.artifact_release.ok_or(BuilderError::Artifact)?;
    let accelerator_program = input.accelerator_program.ok_or(BuilderError::Artifact)?;
    let accelerator_programdata = input
        .accelerator_programdata
        .ok_or(BuilderError::Artifact)?;

    let descriptor =
        CapabilityProgramV4::decode(set.descriptor).map_err(|_| BuilderError::Artifact)?;
    let strategy =
        ExecutionStrategyProgramV2::decode(set.strategy).map_err(|_| BuilderError::Artifact)?;
    let certificate = ExecutionStrategyCertificateV2::decode(certificate_bytes)
        .map_err(|_| BuilderError::Artifact)?;
    let admission = ExecutionStrategyAdmissionV2::decode(admission_bytes)
        .map_err(|_| BuilderError::Artifact)?;
    let release = ArtifactReleaseV1::decode(artifact_bytes).map_err(|_| BuilderError::Artifact)?;
    require_slot_pinned_release_v1(release).map_err(|_| BuilderError::Artifact)?;

    let strategy_id = content(digest(set.strategy))?;
    let certificate_id = content(digest(certificate_bytes))?;
    let admission_id = content(digest(admission_bytes))?;
    let artifact_id =
        ArtifactReleaseIdV1::new(digest(artifact_bytes)).map_err(|_| BuilderError::Artifact)?;
    validate_admitted_aot_v4(
        strategy_id,
        strategy,
        descriptor,
        certificate_id,
        certificate,
        AuthenticatedInterpreterArtifactsV2 {
            account_profile_program: content(digest(set.account_profile))?,
            request_profile_schema: descriptor.request_profile().schema(),
            request_profile_program: content(digest(set.request_profile))?,
            transition_schema: descriptor.transition().schema(),
            transition_program: content(digest(set.transition))?,
            effect_program: content(digest(set.effect))?,
        },
        artifact_id,
        Some((admission_id, admission)),
    )
    .map_err(|_| BuilderError::Artifact)?;
    authenticate_deployment(release, accelerator_program, accelerator_programdata)?;

    Ok(DerivedAdmittedEvidenceV1 {
        certificate: derive_record(
            registry_program,
            EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
            certificate_bytes,
        ),
        admission: derive_record(
            registry_program,
            EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2,
            admission_bytes,
        ),
        artifact_release: derive_record(
            registry_program,
            ARTIFACT_RELEASE_SCHEMA_ID_V1,
            artifact_bytes,
        ),
        accelerator_program: accelerator_program.clone(),
        accelerator_programdata: accelerator_programdata.clone(),
    })
}

/// Derive exact accelerator requests and their release-pinned caller PDAs.
pub fn derive_admitted_authorities_v1(
    input: AdmittedAuthorityInputV1<'_>,
) -> Result<DerivedAdmittedAuthoritiesV1, BuilderError> {
    let scalar_count = u32::try_from(input.scalars.len()).map_err(|_| BuilderError::Arithmetic)?;
    let identity_count =
        u32::try_from(input.identities.len()).map_err(|_| BuilderError::Arithmetic)?;
    let bank_bytes = input
        .scalars
        .len()
        .checked_mul(8)
        .and_then(|scalars| {
            input
                .identities
                .len()
                .checked_mul(32)
                .and_then(|identities| scalars.checked_add(identities))
        })
        .ok_or(BuilderError::Arithmetic)?;
    let mut input_bank = vec![0_u8; bank_bytes];
    encode_register_bank_into(input.scalars, input.identities, &mut input_bank)
        .map_err(|_| BuilderError::Artifact)?;
    let input_bank_digest = content(Sha256::digest(&input_bank).into())?;
    let chunk_count = match classify_bank_transport_v2(scalar_count, identity_count)
        .map_err(|_| BuilderError::Arithmetic)?
    {
        BankTransportV2::InlineReturnData { bank_bytes } => {
            if bank_bytes == 0 || input.transport == RequestTransportV2::ScratchPages {
                return Err(BuilderError::Artifact);
            }
            1_u32
        }
        BankTransportV2::AuthenticatedScratchPages { page_count, .. } => page_count,
    };
    // The host derives what the producer derives: one invocation per output
    // chunk, or exactly one under the output-page transport whatever the bank
    // costs. `accelerator_invocation_count_v2` is the same function Trading and
    // the operator read, so a frame built here and a frame carved there cannot
    // disagree about the span's width.
    let invocation_count =
        accelerator_invocation_count_v2(input.profile, scalar_count, identity_count)
            .map_err(|_| BuilderError::Arithmetic)?;
    let output_page = match input.profile {
        AcceleratorTransportProfileV2::OutputPageV3 => Some(admitted_output_page_address_v1(
            &input.accelerator_program,
            &input.root,
        )),
        AcceleratorTransportProfileV2::ChunkedBankV2
        | AcceleratorTransportProfileV2::ShadowTranscriptV3 => {
            if invocation_count != chunk_count {
                return Err(BuilderError::Artifact);
            }
            None
        }
    };
    let mut entries = Vec::with_capacity(
        usize::try_from(invocation_count).map_err(|_| BuilderError::Arithmetic)?,
    );
    let mut chunk_index = 0_u32;
    while chunk_index < invocation_count {
        let request = match input.profile {
            AcceleratorTransportProfileV2::OutputPageV3 => AcceleratorOutputPageRequestV3::new(
                input.transport,
                input.strategy_program,
                input.certificate_program,
                input.capability_program,
                input.invocation_context,
                input_bank_digest,
                input.tail_count,
                scalar_count,
                identity_count,
                match input.transport {
                    RequestTransportV2::Inline => &input_bank,
                    RequestTransportV2::ScratchPages => &[],
                },
            )
            .map(AdmittedAcceleratorRequestV2::OutputPageV3),
            AcceleratorTransportProfileV2::ChunkedBankV2
            | AcceleratorTransportProfileV2::ShadowTranscriptV3 => AcceleratorRequestV2::new(
                input.transport,
                input.strategy_program,
                input.certificate_program,
                input.capability_program,
                input.invocation_context,
                input_bank_digest,
                input.tail_count,
                scalar_count,
                identity_count,
                chunk_index,
                match input.transport {
                    RequestTransportV2::Inline => &input_bank,
                    RequestTransportV2::ScratchPages => &[],
                },
            )
            .map(AdmittedAcceleratorRequestV2::ChunkedBankV2),
        }
        .map_err(|_| BuilderError::Artifact)?;
        let request_len = request
            .encoded_len()
            .map_err(|_| BuilderError::Arithmetic)?;
        let mut request_bytes = vec![0_u8; request_len];
        request
            .encode_into(&mut request_bytes)
            .map_err(|_| BuilderError::Artifact)?;
        let request_digest = hash(&request_bytes).to_bytes();
        let seeds = CallerAuthoritySeedsV1::new(
            input.release_set,
            input.market.to_bytes(),
            ExecutionRoleV1::Trading,
            input.root.to_bytes(),
            request_digest,
        )
        .map_err(|_| BuilderError::Artifact)?;
        entries.push(DerivedAdmittedAuthorityV1 {
            chunk_index,
            request: request_bytes,
            request_digest,
            authority: Pubkey::find_program_address(&seeds.as_slices(), &input.trading_program).0,
        });
        chunk_index = chunk_index.checked_add(1).ok_or(BuilderError::Arithmetic)?;
    }
    Ok(DerivedAdmittedAuthoritiesV1 {
        input_bank,
        transport: input.transport,
        output_page,
        entries,
    })
}

/// Domain for the address a test or client provisions as an accelerator page.
///
/// NOT A PDA, and it cannot be one: a program-derived address can only be
/// created by its own program, and this account is created by whoever is
/// willing to pay its rent, with a plain `SystemProgram::CreateAccount` that
/// assigns it to the accelerator. What it is instead is a DETERMINISTIC address
/// this harness and its genesis installer both derive the same way, so the page
/// a bundle names and the page a fixture installs cannot drift apart.
pub const ADMITTED_OUTPUT_PAGE_ADDRESS_DOMAIN_V1: &[u8] =
    b"dclutch:test-accelerator-output-page:v3";

/// Derive the deterministic per-root output-page address for this harness.
pub fn admitted_output_page_address_v1(accelerator_program: &Pubkey, root: &Pubkey) -> Pubkey {
    Pubkey::new_from_array(
        hash(
            &[
                ADMITTED_OUTPUT_PAGE_ADDRESS_DOMAIN_V1,
                accelerator_program.as_ref(),
                root.as_ref(),
            ]
            .concat(),
        )
        .to_bytes(),
    )
}

/// Require an observed authority slice to equal the derived request sequence.
/// Missing, extra, reordered and substituted accounts all refuse.
pub fn validate_admitted_authority_keys_v1(
    derived: &DerivedAdmittedAuthoritiesV1,
    observed: &[Pubkey],
) -> Result<(), BuilderError> {
    if observed.len() != derived.entries.len()
        || observed
            .iter()
            .zip(&derived.entries)
            .any(|(key, entry)| *key != entry.authority)
    {
        Err(BuilderError::Binding(line!()))
    } else {
        Ok(())
    }
}

fn authenticate_deployment(
    release: ArtifactReleaseV1,
    program: &BuiltAccountV1,
    programdata: &BuiltAccountV1,
) -> Result<(), BuilderError> {
    let program_view_account = program.chain_view();
    let programdata_view_account = programdata.chain_view();
    if program.key == programdata.key
        || release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || release.program().to_bytes() != program.key.to_bytes()
        || release.programdata() != programdata.key.to_bytes()
        || program_view_account.owner != bpf_loader_upgradeable::ID
        || programdata_view_account.owner != bpf_loader_upgradeable::ID
        || !program_view_account.executable
        || programdata_view_account.executable
    {
        return Err(BuilderError::Artifact);
    }
    let program_view =
        ProgramV3View::parse(&program_view_account.data).map_err(|_| BuilderError::Artifact)?;
    let programdata_view = ProgramDataV3View::parse(&programdata_view_account.data)
        .map_err(|_| BuilderError::Artifact)?;
    if program_view.programdata() != programdata.key.to_bytes() {
        return Err(BuilderError::Artifact);
    }
    let observation = DeploymentObservationV1::new(
        program.key.to_bytes(),
        program_view_account.owner.to_bytes(),
        program_view_account.executable,
        programdata.key.to_bytes(),
        programdata_view_account.owner.to_bytes(),
        programdata_view_account.executable,
        program_view.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        programdata_view.deployment_slot(),
        hash(programdata_view.elf()).to_bytes(),
        programdata_view.upgrade_authority(),
    )
    .map_err(|_| BuilderError::Artifact)?;
    release
        .authenticate_deployment(observation)
        .map_err(|_| BuilderError::Artifact)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_capability_program_contract::v4::{
        ArtifactReferenceV4, CapabilityArtifactsV4, SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
    };
    use dclutch_execution_strategy_contract::v2::{
        ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
        EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, StrategyDispositionV2,
    };
    use dclutch_registry_contract::ArtifactUpgradePolicyV1;
    use dclutch_release_set_contract::ProgramIdentityV1;
    use solana_account::Account;

    fn id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("nonzero identity")
    }

    fn built(key: Pubkey, data: Vec<u8>, executable: bool) -> BuiltAccountV1 {
        BuiltAccountV1 {
            key,
            account: Account {
                lamports: 1,
                data,
                owner: bpf_loader_upgradeable::ID,
                executable,
                rent_epoch: 0,
            },
            observed: None,
        }
    }

    fn loader_program_bytes(programdata: Pubkey) -> Vec<u8> {
        let mut bytes = vec![0_u8; dclutch_registry_svm::LOADER_V3_PROGRAM_BYTES];
        bytes
            .get_mut(0..4)
            .expect("variant")
            .copy_from_slice(&2_u32.to_le_bytes());
        bytes
            .get_mut(4..36)
            .expect("programdata")
            .copy_from_slice(programdata.as_ref());
        bytes
    }

    fn loader_programdata_bytes(slot: u64, elf: &[u8]) -> Vec<u8> {
        let mut bytes =
            vec![0_u8; dclutch_registry_svm::LOADER_V3_PROGRAMDATA_METADATA_BYTES + elf.len()];
        bytes
            .get_mut(0..4)
            .expect("variant")
            .copy_from_slice(&3_u32.to_le_bytes());
        bytes
            .get_mut(4..12)
            .expect("slot")
            .copy_from_slice(&slot.to_le_bytes());
        bytes
            .get_mut(dclutch_registry_svm::LOADER_V3_PROGRAMDATA_METADATA_BYTES..)
            .expect("elf")
            .copy_from_slice(elf);
        bytes
    }

    struct EvidenceFixture {
        descriptor: Vec<u8>,
        account_profile: Vec<u8>,
        request_profile: Vec<u8>,
        transition: Vec<u8>,
        effect: Vec<u8>,
        lifecycle: Vec<u8>,
        strategy: Vec<u8>,
        certificate: Vec<u8>,
        admission: Vec<u8>,
        artifact_release: Vec<u8>,
        program: BuiltAccountV1,
        programdata: BuiltAccountV1,
    }

    impl EvidenceFixture {
        fn set(&self) -> ArtifactSetV1<'_> {
            ArtifactSetV1 {
                descriptor: &self.descriptor,
                account_profile: &self.account_profile,
                request_profile: &self.request_profile,
                transition: &self.transition,
                effect: &self.effect,
                lifecycle: &self.lifecycle,
                strategy: &self.strategy,
                program_set: &[1],
                manifest: &[2],
                config: &[3],
            }
        }

        fn input(&self) -> AdmittedAotInputV1<'_> {
            AdmittedAotInputV1 {
                certificate: Some(&self.certificate),
                admission: Some(&self.admission),
                artifact_release: Some(&self.artifact_release),
                accelerator_program: Some(&self.program),
                accelerator_programdata: Some(&self.programdata),
            }
        }
    }

    fn evidence_fixture() -> EvidenceFixture {
        let program_key = Pubkey::new_from_array([0x81; 32]);
        let programdata_key = Pubkey::new_from_array([0x82; 32]);
        let elf = [0xa5_u8; 128];
        let slot = 77_u64;
        let program = built(program_key, loader_program_bytes(programdata_key), true);
        let programdata = built(programdata_key, loader_programdata_bytes(slot, &elf), false);
        let artifact_release = ArtifactReleaseV1::new(
            ProgramIdentityV1::new(program_key.to_bytes()).expect("program"),
            ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader"),
            programdata_key.to_bytes(),
            id(0x83),
            hash(&elf).to_bytes(),
            slot,
            ArtifactUpgradePolicyV1::Immutable,
            None,
        )
        .expect("release")
        .to_bytes()
        .to_vec();
        let account_profile = vec![0x11];
        let request_profile = vec![0x12];
        let transition = vec![0x13];
        let effect = vec![0x14];
        let lifecycle = vec![0x15];
        let certificate = ExecutionStrategyCertificateV2::new(
            content(digest(&account_profile)).expect("profile"),
            id(0x31),
            content(digest(&request_profile)).expect("request profile"),
            id(0x32),
            content(digest(&transition)).expect("transition"),
            content(digest(&effect)).expect("effect"),
            ArtifactReleaseIdV1::new(digest(&artifact_release)).expect("artifact id"),
            id(0x33),
            id(0x34),
            id(0x35),
        )
        .to_bytes()
        .to_vec();
        let certificate_id = content(digest(&certificate)).expect("certificate id");
        let admission = ExecutionStrategyAdmissionV2::new(certificate_id)
            .to_bytes()
            .to_vec();
        let strategy = ExecutionStrategyProgramV2::new(
            StrategyDispositionV2::AdmittedAot,
            id(0x32),
            content(digest(&transition)).expect("transition"),
            id_from(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2),
            Some(certificate_id),
            id_from(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2),
            Some(content(digest(&admission)).expect("admission")),
            id_from(ACCELERATOR_REQUEST_SCHEMA_ID_V2),
            id_from(ACCELERATOR_ACK_SCHEMA_ID_V2),
        )
        .expect("strategy")
        .to_bytes()
        .to_vec();
        let descriptor = CapabilityProgramV4::new(
            id(0x41),
            id(0x42),
            id(0x43),
            id(0x44),
            content(digest(&lifecycle)).expect("lifecycle"),
            id(0x45),
            CapabilityArtifactsV4 {
                account_profile: ArtifactReferenceV4::new(
                    id(0x30),
                    content(digest(&account_profile)).expect("profile"),
                ),
                request_profile: ArtifactReferenceV4::new(
                    id(0x31),
                    content(digest(&request_profile)).expect("request profile"),
                ),
                lifecycle: ArtifactReferenceV4::new(
                    id_from(SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5),
                    content(digest(&lifecycle)).expect("lifecycle"),
                ),
                strategy: ArtifactReferenceV4::new(
                    id_from(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2),
                    content(digest(&strategy)).expect("strategy"),
                ),
                transition: ArtifactReferenceV4::new(
                    id(0x32),
                    content(digest(&transition)).expect("transition"),
                ),
                effect: ArtifactReferenceV4::new(
                    id(0x36),
                    content(digest(&effect)).expect("effect"),
                ),
            },
            128,
        )
        .expect("descriptor")
        .encode()
        .to_vec();
        EvidenceFixture {
            descriptor,
            account_profile,
            request_profile,
            transition,
            effect,
            lifecycle,
            strategy,
            certificate,
            admission,
            artifact_release,
            program,
            programdata,
        }
    }

    fn id_from(bytes: [u8; 32]) -> ContentId {
        ContentId::new(bytes).expect("schema")
    }

    #[test]
    fn complete_evidence_joins_and_every_missing_or_substituted_owner_refuses() {
        let fixture = evidence_fixture();
        let registry = Pubkey::new_from_array([0x71; 32]);
        assert!(derive_admitted_evidence_v1(registry, fixture.set(), fixture.input()).is_ok());

        let canonical = fixture.input();
        for hostile in [
            AdmittedAotInputV1 {
                certificate: None,
                ..canonical
            },
            AdmittedAotInputV1 {
                admission: None,
                ..canonical
            },
            AdmittedAotInputV1 {
                artifact_release: None,
                ..canonical
            },
            AdmittedAotInputV1 {
                accelerator_program: None,
                ..canonical
            },
            AdmittedAotInputV1 {
                accelerator_programdata: None,
                ..canonical
            },
        ] {
            assert_eq!(
                derive_admitted_evidence_v1(registry, fixture.set(), hostile),
                Err(BuilderError::Artifact)
            );
        }

        let substituted_certificate = ExecutionStrategyCertificateV2::new(
            content(digest(&fixture.account_profile)).expect("profile"),
            id(0x31),
            content(digest(&fixture.request_profile)).expect("request profile"),
            id(0x32),
            content(digest(&fixture.transition)).expect("transition"),
            content(digest(&fixture.effect)).expect("effect"),
            ArtifactReleaseIdV1::new(digest(&fixture.artifact_release)).expect("artifact id"),
            id(0x36),
            id(0x34),
            id(0x35),
        )
        .to_bytes();
        let substituted_admission = ExecutionStrategyAdmissionV2::new(id(0x66)).to_bytes();
        let substituted_release = ArtifactReleaseV1::new(
            ProgramIdentityV1::new(fixture.program.key.to_bytes()).expect("program"),
            ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader"),
            fixture.programdata.key.to_bytes(),
            id(0x84),
            hash(&[0xa5_u8; 128]).to_bytes(),
            77,
            ArtifactUpgradePolicyV1::Immutable,
            None,
        )
        .expect("substituted release")
        .to_bytes();
        let mut substituted_programdata = fixture.programdata.clone();
        *substituted_programdata
            .account
            .data
            .last_mut()
            .expect("ELF byte") ^= 1;
        for hostile in [
            AdmittedAotInputV1 {
                certificate: Some(&substituted_certificate),
                ..canonical
            },
            AdmittedAotInputV1 {
                admission: Some(&substituted_admission),
                ..canonical
            },
            AdmittedAotInputV1 {
                artifact_release: Some(&substituted_release),
                ..canonical
            },
            AdmittedAotInputV1 {
                accelerator_programdata: Some(&substituted_programdata),
                ..canonical
            },
        ] {
            assert_eq!(
                derive_admitted_evidence_v1(registry, fixture.set(), hostile),
                Err(BuilderError::Artifact)
            );
        }
    }

    #[test]
    fn multi_page_requests_derive_one_exact_authority_each() {
        let scalars = vec![7_u64; 120];
        let identities = vec![[8_u8; 32]; 2];
        let input = AdmittedAuthorityInputV1 {
            trading_program: Pubkey::new_from_array([0x51; 32]),
            release_set: id(0x52),
            market: Pubkey::new_from_array([0x53; 32]),
            root: Pubkey::new_from_array([0x54; 32]),
            strategy_program: id(0x55),
            certificate_program: id(0x56),
            capability_program: id(0x57),
            invocation_context: id(0x58),
            transport: RequestTransportV2::ScratchPages,
            profile: AcceleratorTransportProfileV2::ChunkedBankV2,
            accelerator_program: Pubkey::new_from_array([0x59; 32]),
            tail_count: 258,
            scalars: &scalars,
            identities: &identities,
        };
        let derived = derive_admitted_authorities_v1(input).expect("multi-page authorities");
        assert_eq!(derived.transport, RequestTransportV2::ScratchPages);
        assert_eq!(derived.output_page, None);
        assert!(derived.entries.len() > 1);
        for (index, entry) in derived.entries.iter().enumerate() {
            let request = AcceleratorRequestV2::decode(&entry.request).expect("request");
            assert_eq!(
                request.chunk_index(),
                u32::try_from(index).expect("chunk index")
            );
            assert_eq!(request.chunk_count(), derived.entries.len() as u32);
            assert_eq!(hash(&entry.request).to_bytes(), entry.request_digest);
        }
        let exact = derived
            .entries
            .iter()
            .map(|entry| entry.authority)
            .collect::<Vec<_>>();
        assert_eq!(
            validate_admitted_authority_keys_v1(&derived, &exact),
            Ok(())
        );
        assert!(validate_admitted_authority_keys_v1(&derived, &exact[..exact.len() - 1]).is_err());
        let mut substituted = exact;
        substituted[1] = Pubkey::new_from_array([0x99; 32]);
        assert!(validate_admitted_authority_keys_v1(&derived, &substituted).is_err());
    }

    /// The same bank, the same widths, the other transport: one authority, one
    /// page, and a request that carries no chunk coordinate to be wrong about.
    #[test]
    fn the_output_page_transport_derives_one_authority_and_one_page() {
        let scalars = vec![7_u64; 120];
        let identities = vec![[8_u8; 32]; 2];
        let accelerator_program = Pubkey::new_from_array([0x59; 32]);
        let root = Pubkey::new_from_array([0x54; 32]);
        let derived = derive_admitted_authorities_v1(AdmittedAuthorityInputV1 {
            trading_program: Pubkey::new_from_array([0x51; 32]),
            release_set: id(0x52),
            market: Pubkey::new_from_array([0x53; 32]),
            root,
            strategy_program: id(0x55),
            certificate_program: id(0x56),
            capability_program: id(0x57),
            invocation_context: id(0x58),
            transport: RequestTransportV2::ScratchPages,
            profile: AcceleratorTransportProfileV2::OutputPageV3,
            accelerator_program,
            tail_count: 258,
            scalars: &scalars,
            identities: &identities,
        })
        .expect("output-page authorities");
        assert_eq!(derived.entries.len(), 1);
        assert_eq!(
            derived.output_page,
            Some(admitted_output_page_address_v1(&accelerator_program, &root))
        );
        let entry = derived.entries.first().expect("the one entry");
        assert_eq!(entry.chunk_index, 0);
        let request =
            AcceleratorOutputPageRequestV3::decode(&entry.request).expect("output page request");
        assert_eq!(request.total_bank_bytes(), 120 * 8 + 2 * 32);
        assert_eq!(hash(&entry.request).to_bytes(), entry.request_digest);
        // The chunked decoder is not a fallback for it, in either direction.
        assert!(AcceleratorRequestV2::decode(&entry.request).is_err());
    }
}
