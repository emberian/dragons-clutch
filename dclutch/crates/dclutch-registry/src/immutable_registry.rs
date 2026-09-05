//! Pinned current-Registry authorization used by finalized-record readers.
//!
//! The records themselves stay immutable and content-addressed. What decision
//! 0012 generalized is the DEPLOYMENT under them: a Registry release is
//! admitted either because it can never move (`Immutable`) or because it is
//! pinned to the exact deployment slot and exact upgrade authority its
//! activation observed (`ExactAuthority`). Both are checked here by the same
//! `authenticate_deployment`; the second refuses the instant an `Upgrade`
//! moves the slot.

use dclutch_core_contract::ContentId;
use crate::release_set::{ArtifactReleaseIdV1, ProgramIdentityV1};

use crate::{ArtifactReleaseV1, ArtifactUpgradePolicyV1, DeploymentObservationV1, Error, Result};

/// Chain-selected inputs for authenticating the exact current pinned Registry release.
///
/// The SVM adapter must authenticate `finalized_artifact_release_id` as the
/// content digest of the finalized artifact-release record selected by the
/// Core infrastructure profile before constructing this input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PinnedRegistryReleaseInputV1 {
    selected_registry_program: ProgramIdentityV1,
    selected_artifact_release_id: ArtifactReleaseIdV1,
    finalized_artifact_release_id: ArtifactReleaseIdV1,
    release: ArtifactReleaseV1,
    current_deployment: DeploymentObservationV1,
}

impl PinnedRegistryReleaseInputV1 {
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

/// Local semantic authorization for the exact current pinned Registry release.
///
/// This value has no wire encoding and is not a receipt. A composing adapter
/// may use it only in the invocation in which it authenticated the selected
/// finalized release and current Loader deployment.
#[derive(Debug, Eq, PartialEq)]
pub struct AuthenticatedPinnedRegistryReleaseV1 {
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

impl AuthenticatedPinnedRegistryReleaseV1 {
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

/// Authenticate the selected Registry artifact as the exact current pinned deployment.
///
/// This is the prerequisite for the content-addressed finalized-record fast
/// path. A substituted Registry deployment refuses even when all record
/// accounts themselves look canonical, and so does an upgradeable one whose
/// ProgramData no longer carries the exact slot and exact authority the
/// release bound: `authenticate_deployment` names that
/// `ReleaseSupersededByUpgrade`.
pub fn authenticate_pinned_registry_release_v1(
    input: PinnedRegistryReleaseInputV1,
) -> Result<AuthenticatedPinnedRegistryReleaseV1> {
    if input.selected_artifact_release_id != input.finalized_artifact_release_id {
        return Err(Error::RegistryArtifactReleaseMismatch);
    }
    if input.release.program() != input.selected_registry_program {
        return Err(Error::RegistryProgramMismatch);
    }
    require_slot_pinned_release_v1(input.release)?;
    input
        .release
        .authenticate_deployment(input.current_deployment)?;
    Ok(AuthenticatedPinnedRegistryReleaseV1 {
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
    registry: &AuthenticatedPinnedRegistryReleaseV1,
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
/// This is the strict half of the argument, kept for deployments that are
/// genuinely immutable. An upgradeable release is refused here; it reaches the
/// same reuse through [`slot_pinned_release_elf_digest_v1`], which pays for it
/// with a slot equality instead of with irrevocability.
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

/// Return the admitted ELF digest of a deployment whose pin still holds,
/// under either upgrade policy.
///
/// This generalizes [`immutable_release_elf_digest_v1`] per decision 0012
/// (`docs/decisions/0012-devnet-iteration-substrate.md`) and is the single
/// semantic owner of the slot-pin argument:
///
/// - **`Immutable`**: delegated unchanged to
///   [`immutable_release_elf_digest_v1`], which owns the immutability
///   argument — the bytes can never move, so the admitted digest is current.
/// - **`ExactAuthority`**: the Loader V3 writes the current slot into
///   ProgramData on every `Upgrade`, refuses an `Upgrade` in the deployment's
///   own slot ("Program was deployed in this block already"), and a closed
///   program id can never be redeployed. So there is no path to different
///   bytes at this address that leaves `deployment_slot` equal to the slot
///   the release bound: **observed-slot equality proves the admitted digest
///   is the exact current digest**, at zero hashing cost. The observed
///   authority must also equal the bound authority — not for digest
///   soundness (an authority change moves no bytes) but because the release
///   is an identity contract and `authenticate_deployment` would refuse the
///   substitution anyway.
///
/// A refusal here means the pin does not hold — an upgraded substrate
/// ([`Error::ReleaseSupersededByUpgrade`], matching
/// `ArtifactReleaseV1::authenticate_deployment`'s naming for a
/// strictly-later slot), a stale or wrong-generation observation
/// ([`Error::DeploymentSlotMismatch`]), or a changed authority
/// ([`Error::UpgradeAuthorityMismatch`]). Callers refuse; they never fall
/// back to hashing, because on any state a real Loader can reach, hashing
/// succeeds exactly when the pin holds — the fallback would spend a
/// megabyte-scale hash to learn what the pin already said.
///
/// The caller must have observed `observed_upgrade_authority` and
/// `observed_deployment_slot` from the actual ProgramData account in this
/// invocation. Passing a release's own bound values back in would make the
/// check vacuous; every in-tree caller reads them out of a parsed
/// `ProgramDataV3View`.
pub fn slot_pinned_release_elf_digest_v1(
    release: ArtifactReleaseV1,
    observed_upgrade_authority: Option<[u8; crate::IDENTITY_BYTES]>,
    observed_deployment_slot: u64,
) -> Result<[u8; crate::IDENTITY_BYTES]> {
    match release.upgrade_policy() {
        ArtifactUpgradePolicyV1::Immutable => {
            immutable_release_elf_digest_v1(release, observed_upgrade_authority)
        }
        ArtifactUpgradePolicyV1::ExactAuthority => {
            require_slot_pinned_release_v1(release)?;
            if observed_deployment_slot != release.deployment_slot() {
                return Err(release.slot_pin_refusal(observed_deployment_slot));
            }
            if observed_upgrade_authority != release.upgrade_authority() {
                return Err(Error::UpgradeAuthorityMismatch);
            }
            Ok(release.elf_digest())
        }
    }
}

/// Admit one artifact release onto the slot-pinned authentication path.
///
/// Decision 0012 replaced "the release must be `Immutable`" with "the release
/// must be one of the two canonical pinned shapes", because the soundness the
/// readers actually need is supplied by `authenticate_deployment`'s slot and
/// authority equalities, not by irrevocability. Both admitted shapes are:
///
/// - `Immutable` with no bound authority — the deployment cannot move at all;
/// - `ExactAuthority` with an exact bound authority — the deployment can move
///   only by an `Upgrade` signed by that key, and every such move breaks the
///   slot pin and refuses every dependent open market by name.
///
/// A decoded [`ArtifactReleaseV1`] is already canonical in this respect, so
/// this predicate is total on decoded records. It exists so that every reader
/// states its admission out loud in one greppable place rather than by the
/// absence of a check, and so that a hand-constructed release cannot slip a
/// non-canonical pairing past a reader that skipped `decode`.
pub const fn require_slot_pinned_release_v1(release: ArtifactReleaseV1) -> Result<()> {
    match (release.upgrade_policy(), release.upgrade_authority()) {
        (ArtifactUpgradePolicyV1::Immutable, None)
        | (ArtifactUpgradePolicyV1::ExactAuthority, Some(_)) => Ok(()),
        _ => Err(Error::MutableRegistryRelease),
    }
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

    fn authorization() -> AuthenticatedPinnedRegistryReleaseV1 {
        let registry = program(1);
        let release_id = artifact(7);
        let (release, current) = release(registry, ArtifactUpgradePolicyV1::Immutable, None);
        authenticate_pinned_registry_release_v1(PinnedRegistryReleaseInputV1::new(
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
        let authorization = authenticate_pinned_registry_release_v1(
            PinnedRegistryReleaseInputV1::new(registry, release_id, release_id, release, current),
        )
        .expect("immutable current Registry");
        assert_eq!(authorization.registry_program(), registry);
        assert_eq!(authorization.artifact_release_id(), release_id);
        assert_eq!(authorization.semantic_release_id(), content(5));
    }

    /// Decision 0012 moved this test's first arm, and did not delete it.
    ///
    /// Before 0012 an upgradeable Registry release refused on sight. It is now
    /// admitted while its pin holds -- and an `Immutable` release over a
    /// ProgramData that RETAINED an authority still refuses on sight, because
    /// that release's own policy claims an irrevocability the chain contradicts.
    /// The substitution refusals below are unchanged.
    #[test]
    fn unpinned_or_substituted_registry_refuses() {
        let registry = program(1);
        let release_id = artifact(7);
        let (immutable_release, mutable_observation) = {
            let (value, _) = release(registry, ArtifactUpgradePolicyV1::Immutable, None);
            let observation = DeploymentObservationV1::new(
                registry.to_bytes(),
                value.loader_program().to_bytes(),
                true,
                value.programdata(),
                value.loader_program().to_bytes(),
                false,
                value.programdata(),
                value.loader_program().to_bytes(),
                value.deployment_slot(),
                value.elf_digest(),
                Some([8; 32]),
            )
            .expect("retained-authority observation");
            (value, observation)
        };
        assert_eq!(
            authenticate_pinned_registry_release_v1(PinnedRegistryReleaseInputV1::new(
                registry,
                release_id,
                release_id,
                immutable_release,
                mutable_observation,
            ),),
            Err(Error::UpgradeAuthorityMismatch)
        );

        let (immutable, current) = release(registry, ArtifactUpgradePolicyV1::Immutable, None);
        assert_eq!(
            authenticate_pinned_registry_release_v1(PinnedRegistryReleaseInputV1::new(
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
            authenticate_pinned_registry_release_v1(PinnedRegistryReleaseInputV1::new(
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

    /// Decision 0012: slot equality buys back what immutability was buying.
    ///
    /// The `Immutable` arm is unchanged and delegates. The `ExactAuthority` arm
    /// is the new one, and it is admitted on exactly two equalities: the exact
    /// bound authority, and the exact bound deployment slot. Everything else in
    /// the neighbourhood refuses, and a strictly LATER slot refuses under its
    /// own operator-actionable name.
    #[test]
    fn slot_pinned_digest_reuses_on_the_pin_and_refuses_every_way_off_it() {
        let registry = program(1);
        let (immutable, _) = release(registry, ArtifactUpgradePolicyV1::Immutable, None);
        let pinned_slot = immutable.deployment_slot();

        // The immutable arm delegates unchanged, at any observed slot: nothing
        // can move those bytes, so the slot is `authenticate_deployment`'s
        // business and not the digest argument's.
        assert_eq!(
            slot_pinned_release_elf_digest_v1(immutable, None, pinned_slot),
            Ok(immutable.elf_digest())
        );
        assert_eq!(
            slot_pinned_release_elf_digest_v1(immutable, Some([9; 32]), pinned_slot),
            Err(Error::MutableRegistryRelease)
        );

        let authority = [7_u8; 32];
        let (upgradeable, _) = release(
            registry,
            ArtifactUpgradePolicyV1::ExactAuthority,
            Some(authority),
        );

        // POSITIVE: the pin holds -- the admitted digest is the current digest.
        assert_eq!(
            slot_pinned_release_elf_digest_v1(upgradeable, Some(authority), pinned_slot),
            Ok(upgradeable.elf_digest())
        );

        // The upgrade lands: a strictly later slot, named for the operator.
        assert_eq!(
            slot_pinned_release_elf_digest_v1(upgradeable, Some(authority), pinned_slot + 1),
            Err(Error::ReleaseSupersededByUpgrade)
        );

        // A slot BELOW the pin is not an upgrade -- the Loader only ever writes
        // the current slot -- so it keeps the substitution name.
        assert_eq!(
            slot_pinned_release_elf_digest_v1(upgradeable, Some(authority), pinned_slot - 1),
            Err(Error::DeploymentSlotMismatch)
        );

        // HOSTILE: pin substitution. A different authority at the pinned slot.
        assert_eq!(
            slot_pinned_release_elf_digest_v1(upgradeable, Some([8; 32]), pinned_slot),
            Err(Error::UpgradeAuthorityMismatch)
        );

        // HOSTILE: a revoked authority at the pinned slot. `SetAuthority` moves
        // no slot, so only the identity contract catches this one.
        assert_eq!(
            slot_pinned_release_elf_digest_v1(upgradeable, None, pinned_slot),
            Err(Error::UpgradeAuthorityMismatch)
        );
    }

    /// The admission predicate names the two canonical pinned shapes.
    #[test]
    fn slot_pinned_admission_accepts_both_canonical_shapes() {
        let registry = program(1);
        let (immutable, _) = release(registry, ArtifactUpgradePolicyV1::Immutable, None);
        let (upgradeable, _) = release(
            registry,
            ArtifactUpgradePolicyV1::ExactAuthority,
            Some([7; 32]),
        );
        assert_eq!(require_slot_pinned_release_v1(immutable), Ok(()));
        assert_eq!(require_slot_pinned_release_v1(upgradeable), Ok(()));
    }

    /// An upgradeable Registry release is authenticated on its pin, and a moved
    /// slot flips the whole finalized-record fast path to a named refusal.
    #[test]
    fn pinned_registry_release_admits_a_mutable_substrate_until_it_moves() {
        let registry = program(1);
        let authority = [7_u8; 32];
        let (release_value, observation) = release(
            registry,
            ArtifactUpgradePolicyV1::ExactAuthority,
            Some(authority),
        );
        let release_id = artifact(7);
        assert!(
            authenticate_pinned_registry_release_v1(PinnedRegistryReleaseInputV1::new(
                registry,
                release_id,
                release_id,
                release_value,
                observation,
            ))
            .is_ok()
        );

        let upgraded = DeploymentObservationV1::new(
            registry.to_bytes(),
            release_value.loader_program().to_bytes(),
            true,
            release_value.programdata(),
            release_value.loader_program().to_bytes(),
            false,
            release_value.programdata(),
            release_value.loader_program().to_bytes(),
            release_value.deployment_slot() + 1,
            release_value.elf_digest(),
            Some(authority),
        )
        .expect("upgraded observation");
        assert_eq!(
            authenticate_pinned_registry_release_v1(PinnedRegistryReleaseInputV1::new(
                registry,
                release_id,
                release_id,
                release_value,
                upgraded,
            )),
            Err(Error::ReleaseSupersededByUpgrade)
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
