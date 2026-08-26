//! Immutable current-Registry authorization used by finalized-record readers.

use dclutch_core_contract::ContentId;
use dclutch_release_set_contract::{ArtifactReleaseIdV1, ProgramIdentityV1};

use crate::{ArtifactReleaseV1, ArtifactUpgradePolicyV1, DeploymentObservationV1, Error, Result};

/// Chain-selected inputs for authenticating the exact current immutable Registry release.
///
/// The SVM adapter must authenticate `finalized_artifact_release_id` as the
/// content digest of the finalized artifact-release record selected by the
/// Core infrastructure profile before constructing this input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImmutableRegistryReleaseInputV1 {
    selected_registry_program: ProgramIdentityV1,
    selected_artifact_release_id: ArtifactReleaseIdV1,
    finalized_artifact_release_id: ArtifactReleaseIdV1,
    release: ArtifactReleaseV1,
    current_deployment: DeploymentObservationV1,
}

impl ImmutableRegistryReleaseInputV1 {
    /// Construct one complete chain-derived Registry-release observation.
    pub const fn new(
        selected_registry_program: ProgramIdentityV1,
        selected_artifact_release_id: ArtifactReleaseIdV1,
        finalized_artifact_release_id: ArtifactReleaseIdV1,
        release: ArtifactReleaseV1,
        current_deployment: DeploymentObservationV1,
    ) -> Self {
        Self {
            selected_registry_program,
            selected_artifact_release_id,
            finalized_artifact_release_id,
            release,
            current_deployment,
        }
    }
}

/// Local semantic authorization for the exact current immutable Registry release.
///
/// This value has no wire encoding and is not a receipt. A composing adapter
/// may use it only in the invocation in which it authenticated the selected
/// finalized release and current Loader deployment.
#[derive(Debug, Eq, PartialEq)]
pub struct AuthenticatedImmutableRegistryReleaseV1 {
    registry_program: ProgramIdentityV1,
    artifact_release_id: ArtifactReleaseIdV1,
    semantic_release_id: ContentId,
}

/// Exact content-addressed record identity selected by a semantic reader.
///
/// The SVM adapter derives both observed account addresses from `schema_id`
/// and `content_digest` under the authenticated Registry program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImmutableFinalizedRecordExpectationV1 {
    schema_id: ContentId,
    content_digest: ContentId,
    exact_data_length: u64,
    raw_rent_minimum: u64,
}

impl ImmutableFinalizedRecordExpectationV1 {
    /// Construct one adapter-derived finalized-record expectation.
    pub fn new(
        schema_id: ContentId,
        content_digest: ContentId,
        exact_data_length: u64,
        raw_rent_minimum: u64,
    ) -> Result<Self> {
        if exact_data_length == 0 || raw_rent_minimum == 0 {
            return Err(Error::InvalidFinalizedRecordShape);
        }
        Ok(Self {
            schema_id,
            content_digest,
            exact_data_length,
            raw_rent_minimum,
        })
    }

    /// Selected finalized-record schema identity.
    pub const fn schema_id(self) -> ContentId {
        self.schema_id
    }

    /// Selected complete-body content digest.
    pub const fn content_digest(self) -> ContentId {
        self.content_digest
    }

    /// Exact selected raw-record width.
    pub const fn exact_data_length(self) -> u64 {
        self.exact_data_length
    }

    /// Current chain-derived Rent minimum for the selected width.
    pub const fn raw_rent_minimum(self) -> u64 {
        self.raw_rent_minimum
    }
}

/// Read-only account facts for recurring immutable finalized-record use.
///
/// This deliberately carries no record bytes and no caller-supplied
/// "hash-valid" flag.  The full body hash is checked by the immutable Registry
/// at finalization.  Once that exact Registry release is authenticated, the
/// content-addressed raw PDA and absent staging cursor make the body immutable;
/// recurring readers still recheck ownership, Rent, width, and privileges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImmutableFinalizedRecordObservationV1 {
    raw_record_account: [u8; 32],
    raw_owner: ProgramIdentityV1,
    raw_lamports: u64,
    raw_data_length: u64,
    raw_is_signer: bool,
    raw_is_writable: bool,
    raw_executable: bool,
    staging_account: [u8; 32],
    staging_owner: ProgramIdentityV1,
    staging_data_length: u64,
    staging_is_signer: bool,
    staging_is_writable: bool,
    staging_executable: bool,
}

impl ImmutableFinalizedRecordObservationV1 {
    /// Construct one complete read-only account observation.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        raw_record_account: [u8; 32],
        raw_owner: ProgramIdentityV1,
        raw_lamports: u64,
        raw_data_length: u64,
        raw_is_signer: bool,
        raw_is_writable: bool,
        raw_executable: bool,
        staging_account: [u8; 32],
        staging_owner: ProgramIdentityV1,
        staging_data_length: u64,
        staging_is_signer: bool,
        staging_is_writable: bool,
        staging_executable: bool,
    ) -> Self {
        Self {
            raw_record_account,
            raw_owner,
            raw_lamports,
            raw_data_length,
            raw_is_signer,
            raw_is_writable,
            raw_executable,
            staging_account,
            staging_owner,
            staging_data_length,
            staging_is_signer,
            staging_is_writable,
            staging_executable,
        }
    }
}

/// Exact PDA and System-owner check requested from the SVM adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImmutableFinalizedRecordObligationV1 {
    registry_program: ProgramIdentityV1,
    schema_id: ContentId,
    content_digest: ContentId,
    raw_record_account: [u8; 32],
    staging_account: [u8; 32],
    staging_owner: ProgramIdentityV1,
}

impl ImmutableFinalizedRecordObligationV1 {
    /// Exact authenticated immutable Registry program.
    pub const fn registry_program(self) -> ProgramIdentityV1 {
        self.registry_program
    }

    /// Selected semantic schema seed.
    pub const fn schema_id(self) -> ContentId {
        self.schema_id
    }

    /// Selected complete-body digest seed.
    pub const fn content_digest(self) -> ContentId {
        self.content_digest
    }

    /// Observed raw-record account claimed as the canonical PDA.
    pub const fn raw_record_account(self) -> [u8; 32] {
        self.raw_record_account
    }

    /// Observed staging account claimed as the canonical PDA.
    pub const fn staging_account(self) -> [u8; 32] {
        self.staging_account
    }

    /// Observed staging owner claimed as the canonical System Program.
    pub const fn staging_owner(self) -> ProgramIdentityV1 {
        self.staging_owner
    }
}

/// Explicit SVM trust boundary for immutable finalized-record coordinates.
///
/// An implementation derives both PDAs with the canonical Registry record
/// seed domains under `registry_program` and compares `staging_owner` with the
/// canonical System Program. It must not accept caller-provided derived
/// addresses or a caller-selected System Program as authority.
pub trait ImmutableFinalizedRecordAdapterV1 {
    /// Validate the exact content-addressed PDAs and canonical staging owner.
    fn validate_record_obligation(&self, obligation: &ImmutableFinalizedRecordObligationV1)
    -> bool;
}

/// Invocation-scoped authorization for one immutable finalized Registry record.
///
/// The value has no wire encoding and proves only the named schema, digest,
/// account coordinates, and width.  A semantic decoder must still decode the
/// borrowed raw bytes under `schema_id`; this token merely makes a second
/// complete-body hash unnecessary in that same invocation.
#[derive(Debug, Eq, PartialEq)]
pub struct AuthenticatedImmutableFinalizedRecordV1 {
    registry_program: ProgramIdentityV1,
    schema_id: ContentId,
    content_digest: ContentId,
    raw_record_account: [u8; 32],
    exact_data_length: u64,
}

impl AuthenticatedImmutableFinalizedRecordV1 {
    /// Exact immutable Registry program owning the raw record.
    pub const fn registry_program(&self) -> ProgramIdentityV1 {
        self.registry_program
    }

    /// Exact semantic schema selected by the reader.
    pub const fn schema_id(&self) -> ContentId {
        self.schema_id
    }

    /// Complete-body digest embedded in the content-addressed PDA.
    pub const fn content_digest(&self) -> ContentId {
        self.content_digest
    }

    /// Exact authenticated raw-record account.
    pub const fn raw_record_account(&self) -> [u8; 32] {
        self.raw_record_account
    }

    /// Exact authenticated raw-record byte width.
    pub const fn exact_data_length(&self) -> u64 {
        self.exact_data_length
    }
}

impl AuthenticatedImmutableRegistryReleaseV1 {
    /// Return the exact immutable Registry program identity.
    pub const fn registry_program(&self) -> ProgramIdentityV1 {
        self.registry_program
    }

    /// Return the selected finalized Registry artifact-release identity.
    pub const fn artifact_release_id(&self) -> ArtifactReleaseIdV1 {
        self.artifact_release_id
    }

    /// Return the Registry semantic release implemented by the current ELF.
    pub const fn semantic_release_id(&self) -> ContentId {
        self.semantic_release_id
    }
}

/// Authenticate the selected Registry artifact as the exact current immutable deployment.
///
/// This is the prerequisite for the content-addressed finalized-record fast
/// path. Upgradeable or substituted Registry deployments refuse even when all
/// record accounts themselves look canonical.
pub fn authenticate_immutable_registry_release_v1(
    input: ImmutableRegistryReleaseInputV1,
) -> Result<AuthenticatedImmutableRegistryReleaseV1> {
    if input.selected_artifact_release_id != input.finalized_artifact_release_id {
        return Err(Error::RegistryArtifactReleaseMismatch);
    }
    if input.release.program() != input.selected_registry_program {
        return Err(Error::RegistryProgramMismatch);
    }
    if input.release.upgrade_policy() != ArtifactUpgradePolicyV1::Immutable
        || input.release.upgrade_authority().is_some()
    {
        return Err(Error::MutableRegistryRelease);
    }
    input
        .release
        .authenticate_deployment(input.current_deployment)?;
    Ok(AuthenticatedImmutableRegistryReleaseV1 {
        registry_program: input.selected_registry_program,
        artifact_release_id: input.selected_artifact_release_id,
        semantic_release_id: input.release.semantic_release_id(),
    })
}

/// Authenticate one recurring read of a record finalized by the exact immutable Registry.
///
/// The adapter must derive the expectation PDAs with the canonical Registry
/// record seed domains and must pass the canonical System Program identity.
/// Nonzero lamports on an otherwise vacant System-owned staging PDA are
/// accepted as unclassified dust; they do not recreate Registry-owned staging
/// state.
pub fn authenticate_immutable_finalized_record_v1<A: ImmutableFinalizedRecordAdapterV1>(
    adapter: &A,
    registry: &AuthenticatedImmutableRegistryReleaseV1,
    expectation: ImmutableFinalizedRecordExpectationV1,
    observation: ImmutableFinalizedRecordObservationV1,
) -> Result<AuthenticatedImmutableFinalizedRecordV1> {
    if observation.raw_record_account.iter().all(|byte| *byte == 0)
        || observation.staging_account.iter().all(|byte| *byte == 0)
        || observation.raw_record_account == observation.staging_account
    {
        return Err(Error::NonCanonicalRecordCoordinate);
    }
    let obligation = ImmutableFinalizedRecordObligationV1 {
        registry_program: registry.registry_program,
        schema_id: expectation.schema_id,
        content_digest: expectation.content_digest,
        raw_record_account: observation.raw_record_account,
        staging_account: observation.staging_account,
        staging_owner: observation.staging_owner,
    };
    if !adapter.validate_record_obligation(&obligation) {
        return Err(Error::NonCanonicalRecordCoordinate);
    }
    if observation.raw_owner != registry.registry_program {
        return Err(Error::FinalizedRecordOwnerMismatch);
    }
    if observation.raw_lamports < expectation.raw_rent_minimum {
        return Err(Error::FinalizedRecordRentDeficit);
    }
    if observation.raw_data_length != expectation.exact_data_length
        || observation.raw_is_signer
        || observation.raw_is_writable
        || observation.raw_executable
    {
        return Err(Error::InvalidFinalizedRecordShape);
    }
    if observation.staging_data_length != 0
        || observation.staging_is_signer
        || observation.staging_is_writable
        || observation.staging_executable
    {
        return Err(Error::StagingCursorPresent);
    }
    Ok(AuthenticatedImmutableFinalizedRecordV1 {
        registry_program: registry.registry_program,
        schema_id: expectation.schema_id,
        content_digest: expectation.content_digest,
        raw_record_account: observation.raw_record_account,
        exact_data_length: expectation.exact_data_length,
    })
}

/// Return the exact current ELF digest of an already-immutable deployment.
///
/// Activation hashed the complete ELF once, before persisting `release`. A
/// Loader V3 deployment whose admitted policy is `Immutable`, whose release
/// carries no upgrade authority, and whose observed ProgramData currently
/// carries no upgrade authority can never be redeployed, so the admitted
/// digest is the exact current ELF digest. Re-hashing a multi-hundred-kilobyte
/// ELF on every recurring action therefore recomputes an authenticated fact.
///
/// This is the single semantic owner of that argument. An upgradeable release
/// has no such guarantee: it is refused here and must hash the observed ELF.
/// `authenticate_deployment` still checks identity, link, ownership,
/// executability, the exact deployment slot, and the upgrade authority.
pub fn immutable_release_elf_digest_v1(
    release: ArtifactReleaseV1,
    observed_upgrade_authority: Option<[u8; crate::IDENTITY_BYTES]>,
) -> Result<[u8; crate::IDENTITY_BYTES]> {
    if release.upgrade_policy() != ArtifactUpgradePolicyV1::Immutable
        || release.upgrade_authority().is_some()
        || observed_upgrade_authority.is_some()
    {
        return Err(Error::MutableRegistryRelease);
    }
    Ok(release.elf_digest())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ArtifactReleaseV1, ArtifactUpgradePolicyV1, DeploymentObservationV1};

    fn program(fill: u8) -> ProgramIdentityV1 {
        ProgramIdentityV1::decode(&[fill; 32]).expect("program identity")
    }

    fn artifact(fill: u8) -> ArtifactReleaseIdV1 {
        ArtifactReleaseIdV1::decode(&[fill; 32]).expect("artifact identity")
    }

    fn content(fill: u8) -> ContentId {
        ContentId::new([fill; 32]).expect("content identity")
    }

    fn release(
        registry: ProgramIdentityV1,
        policy: ArtifactUpgradePolicyV1,
        authority: Option<[u8; 32]>,
    ) -> (ArtifactReleaseV1, DeploymentObservationV1) {
        let loader = program(2);
        let programdata = [3; 32];
        let elf = [4; 32];
        let value = ArtifactReleaseV1::new(
            registry,
            loader,
            programdata,
            content(5),
            elf,
            6,
            policy,
            authority,
        )
        .expect("artifact release");
        let observation = DeploymentObservationV1::new(
            registry.to_bytes(),
            loader.to_bytes(),
            true,
            programdata,
            loader.to_bytes(),
            false,
            programdata,
            loader.to_bytes(),
            6,
            elf,
            authority,
        )
        .expect("deployment observation");
        (value, observation)
    }

    fn authorization() -> AuthenticatedImmutableRegistryReleaseV1 {
        let registry = program(1);
        let release_id = artifact(7);
        let (release, current) = release(registry, ArtifactUpgradePolicyV1::Immutable, None);
        authenticate_immutable_registry_release_v1(ImmutableRegistryReleaseInputV1::new(
            registry, release_id, release_id, release, current,
        ))
        .expect("immutable Registry authorization")
    }

    #[derive(Clone, Copy)]
    struct ExactRecordAdapter {
        expected: ImmutableFinalizedRecordObligationV1,
    }

    impl ImmutableFinalizedRecordAdapterV1 for ExactRecordAdapter {
        fn validate_record_obligation(
            &self,
            obligation: &ImmutableFinalizedRecordObligationV1,
        ) -> bool {
            obligation == &self.expected
        }
    }

    fn record_fixture() -> (
        ImmutableFinalizedRecordExpectationV1,
        ImmutableFinalizedRecordObservationV1,
        ExactRecordAdapter,
    ) {
        let expectation =
            ImmutableFinalizedRecordExpectationV1::new(content(11), content(12), 640, 2_000)
                .expect("record expectation");
        let observation = ImmutableFinalizedRecordObservationV1::new(
            [13; 32],
            program(1),
            2_000,
            640,
            false,
            false,
            false,
            [14; 32],
            program(15),
            0,
            false,
            false,
            false,
        );
        let adapter = ExactRecordAdapter {
            expected: ImmutableFinalizedRecordObligationV1 {
                registry_program: program(1),
                schema_id: content(11),
                content_digest: content(12),
                raw_record_account: [13; 32],
                staging_account: [14; 32],
                staging_owner: program(15),
            },
        };
        (expectation, observation, adapter)
    }

    #[test]
    fn exact_immutable_current_registry_is_authorized() {
        let registry = program(1);
        let release_id = artifact(7);
        let (release, current) = release(registry, ArtifactUpgradePolicyV1::Immutable, None);
        let authorization =
            authenticate_immutable_registry_release_v1(ImmutableRegistryReleaseInputV1::new(
                registry, release_id, release_id, release, current,
            ))
            .expect("immutable current Registry");
        assert_eq!(authorization.registry_program(), registry);
        assert_eq!(authorization.artifact_release_id(), release_id);
        assert_eq!(authorization.semantic_release_id(), content(5));
    }

    #[test]
    fn mutable_or_substituted_registry_refuses() {
        let registry = program(1);
        let release_id = artifact(7);
        let authority = Some([8; 32]);
        let (mutable, mutable_current) =
            release(registry, ArtifactUpgradePolicyV1::ExactAuthority, authority);
        assert_eq!(
            authenticate_immutable_registry_release_v1(ImmutableRegistryReleaseInputV1::new(
                registry,
                release_id,
                release_id,
                mutable,
                mutable_current,
            ),),
            Err(Error::MutableRegistryRelease)
        );

        let (immutable, current) = release(registry, ArtifactUpgradePolicyV1::Immutable, None);
        assert_eq!(
            authenticate_immutable_registry_release_v1(ImmutableRegistryReleaseInputV1::new(
                registry,
                release_id,
                artifact(9),
                immutable,
                current,
            ),),
            Err(Error::RegistryArtifactReleaseMismatch)
        );

        let (immutable, current) = release(registry, ArtifactUpgradePolicyV1::Immutable, None);
        assert_eq!(
            authenticate_immutable_registry_release_v1(ImmutableRegistryReleaseInputV1::new(
                program(10),
                release_id,
                release_id,
                immutable,
                current,
            ),),
            Err(Error::RegistryProgramMismatch)
        );
    }

    #[test]
    fn immutable_finalized_record_fast_path_binds_every_selected_fact() {
        let registry = authorization();
        let (expectation, observation, adapter) = record_fixture();
        let record = authenticate_immutable_finalized_record_v1(
            &adapter,
            &registry,
            expectation,
            observation,
        )
        .expect("immutable finalized record");
        assert_eq!(record.registry_program(), program(1));
        assert_eq!(record.schema_id(), content(11));
        assert_eq!(record.content_digest(), content(12));
        assert_eq!(record.raw_record_account(), [13; 32]);
        assert_eq!(record.exact_data_length(), 640);
    }

    #[test]
    fn schema_digest_and_pda_substitution_refuse() {
        let registry = authorization();
        let (expectation, observation, adapter) = record_fixture();
        for substituted in [
            ImmutableFinalizedRecordExpectationV1::new(content(21), content(12), 640, 2_000)
                .expect("schema substitute"),
            ImmutableFinalizedRecordExpectationV1::new(content(11), content(22), 640, 2_000)
                .expect("digest substitute"),
        ] {
            assert_eq!(
                authenticate_immutable_finalized_record_v1(
                    &adapter,
                    &registry,
                    substituted,
                    observation,
                ),
                Err(Error::NonCanonicalRecordCoordinate)
            );
        }

        let substituted_raw = ImmutableFinalizedRecordObservationV1::new(
            [23; 32],
            observation.raw_owner,
            observation.raw_lamports,
            observation.raw_data_length,
            false,
            false,
            false,
            observation.staging_account,
            observation.staging_owner,
            0,
            false,
            false,
            false,
        );
        assert_eq!(
            authenticate_immutable_finalized_record_v1(
                &adapter,
                &registry,
                expectation,
                substituted_raw,
            ),
            Err(Error::NonCanonicalRecordCoordinate)
        );
    }

    #[test]
    fn owner_rent_width_and_mutable_privileges_refuse() {
        let registry = authorization();
        let (expectation, observation, adapter) = record_fixture();
        let cases = [
            (
                ImmutableFinalizedRecordObservationV1::new(
                    observation.raw_record_account,
                    program(31),
                    2_000,
                    640,
                    false,
                    false,
                    false,
                    observation.staging_account,
                    observation.staging_owner,
                    0,
                    false,
                    false,
                    false,
                ),
                Error::FinalizedRecordOwnerMismatch,
            ),
            (
                ImmutableFinalizedRecordObservationV1::new(
                    observation.raw_record_account,
                    observation.raw_owner,
                    1_999,
                    640,
                    false,
                    false,
                    false,
                    observation.staging_account,
                    observation.staging_owner,
                    0,
                    false,
                    false,
                    false,
                ),
                Error::FinalizedRecordRentDeficit,
            ),
            (
                ImmutableFinalizedRecordObservationV1::new(
                    observation.raw_record_account,
                    observation.raw_owner,
                    2_000,
                    639,
                    false,
                    false,
                    false,
                    observation.staging_account,
                    observation.staging_owner,
                    0,
                    false,
                    false,
                    false,
                ),
                Error::InvalidFinalizedRecordShape,
            ),
            (
                ImmutableFinalizedRecordObservationV1::new(
                    observation.raw_record_account,
                    observation.raw_owner,
                    2_000,
                    640,
                    false,
                    true,
                    false,
                    observation.staging_account,
                    observation.staging_owner,
                    0,
                    false,
                    false,
                    false,
                ),
                Error::InvalidFinalizedRecordShape,
            ),
        ];
        for (hostile, error) in cases {
            assert_eq!(
                authenticate_immutable_finalized_record_v1(
                    &adapter,
                    &registry,
                    expectation,
                    hostile,
                ),
                Err(error)
            );
        }
    }

    #[test]
    fn live_or_substituted_staging_refuses_but_system_dust_is_irrelevant() {
        let registry = authorization();
        let (expectation, observation, adapter) = record_fixture();
        for hostile in [
            ImmutableFinalizedRecordObservationV1::new(
                observation.raw_record_account,
                observation.raw_owner,
                2_000,
                640,
                false,
                false,
                false,
                observation.staging_account,
                observation.staging_owner,
                1,
                false,
                false,
                false,
            ),
            ImmutableFinalizedRecordObservationV1::new(
                observation.raw_record_account,
                observation.raw_owner,
                2_000,
                640,
                false,
                false,
                false,
                observation.staging_account,
                observation.staging_owner,
                0,
                false,
                true,
                false,
            ),
        ] {
            assert_eq!(
                authenticate_immutable_finalized_record_v1(
                    &adapter,
                    &registry,
                    expectation,
                    hostile,
                ),
                Err(Error::StagingCursorPresent)
            );
        }

        let wrong_owner = ImmutableFinalizedRecordObservationV1::new(
            observation.raw_record_account,
            observation.raw_owner,
            2_000,
            640,
            false,
            false,
            false,
            observation.staging_account,
            program(41),
            0,
            false,
            false,
            false,
        );
        assert_eq!(
            authenticate_immutable_finalized_record_v1(
                &adapter,
                &registry,
                expectation,
                wrong_owner,
            ),
            Err(Error::NonCanonicalRecordCoordinate)
        );
    }

    #[test]
    fn immutable_elf_digest_is_the_admitted_digest_and_upgradeable_refuses() {
        let registry = program(1);
        let (immutable, _) = release(registry, ArtifactUpgradePolicyV1::Immutable, None);
        assert_eq!(
            immutable_release_elf_digest_v1(immutable, None),
            Ok(immutable.elf_digest())
        );
        // A live upgrade authority on the observed ProgramData refuses even
        // when the admitted release claims immutability.
        assert_eq!(
            immutable_release_elf_digest_v1(immutable, Some([9; 32])),
            Err(Error::MutableRegistryRelease)
        );
        let (upgradeable, _) = release(
            registry,
            ArtifactUpgradePolicyV1::ExactAuthority,
            Some([7; 32]),
        );
        assert_eq!(
            immutable_release_elf_digest_v1(upgradeable, Some([7; 32])),
            Err(Error::MutableRegistryRelease)
        );
        assert_eq!(
            immutable_release_elf_digest_v1(upgradeable, None),
            Err(Error::MutableRegistryRelease)
        );
    }

    #[test]
    fn zero_width_or_rent_expectation_refuses() {
        assert_eq!(
            ImmutableFinalizedRecordExpectationV1::new(content(1), content(2), 0, 1),
            Err(Error::InvalidFinalizedRecordShape)
        );
        assert_eq!(
            ImmutableFinalizedRecordExpectationV1::new(content(1), content(2), 1, 0),
            Err(Error::InvalidFinalizedRecordShape)
        );
    }
}
