#![allow(clippy::indexing_slicing)]

extern crate std;

use dclutch_core_contract::ContentId;
use crate::release_set::{
    ArtifactReleaseIdV1, EXECUTION_ROLE_COUNT_V1, EXECUTION_ROLE_ORDER_V1, ExecutionReleaseSetV1,
    ExecutionRoleBindingV1, ExecutionRoleV1, ProgramIdentityV1,
};

use crate::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATED_EXECUTION_RELEASE_SET_MAGIC_V1,
    ACTIVATED_EXECUTION_RELEASE_SET_PROFILE_V1, ACTIVATED_EXECUTION_RELEASE_SET_SCHEMA_VERSION_V1,
    ACTIVATION_PDA_DOMAIN_V1, ARTIFACT_RELEASE_BYTES_V1, ARTIFACT_RELEASE_MAGIC_V1,
    ActivatedExecutionReleaseSetV1, ActivatedExecutionReleaseSetViewV1, ArtifactActivationInputV1,
    ArtifactReleaseV1, ArtifactUpgradePolicyV1, DeploymentObservationV1, Error,
    ExecutionReleaseActivationInputsV1, LINEAGE_WALK_MAX_HOPS_V1, LineageAt, LineageWalkRefusal,
    RELEASE_LINEAGE_BYTES_V1, RELEASE_LINEAGE_MAGIC_V1, RELEASE_LINEAGE_PDA_DOMAIN_V1,
    RELEASE_LINEAGE_PDA_SEED_COUNT_V1, RELEASE_LINEAGE_PROFILE_V1,
    RELEASE_LINEAGE_SCHEMA_VERSION_V1, ReleaseLineageV1, activate_execution_release_set_v1,
    activate_execution_role_into_v1, activation_cache_progress_v1, initialize_activation_cache_v1,
    put_activation_cache_bump_v1, walk_lineage_to, walk_lineage_to_head,
};

const ROLE_CACHE_HEADER_BYTES: usize = 48;
const ROLE_CACHE_BYTES: usize = 32 + ARTIFACT_RELEASE_BYTES_V1;
const ARTIFACT_DEPLOYMENT_SLOT_OFFSET: usize = 176;

fn bytes(seed: u8) -> [u8; 32] {
    [seed; 32]
}

fn content(seed: u8) -> ContentId {
    ContentId::new(bytes(seed)).expect("nonzero content identity")
}

fn program(seed: u8) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(bytes(seed)).expect("nonzero program identity")
}

fn artifact_id(seed: u8) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(bytes(seed)).expect("nonzero release identity")
}

fn copied<T: Copy, const N: usize>(values: &[T; N], index: usize) -> T {
    values
        .get(index)
        .copied()
        .expect("fixture index is in range")
}

fn flip_at<const N: usize>(bytes: &mut [u8; N], offset: usize) {
    let byte = bytes.get_mut(offset).expect("hostile offset is in range");
    *byte ^= 0xff;
}

fn zero_range<const N: usize>(bytes: &mut [u8; N], offset: usize, width: usize) {
    let end = offset.checked_add(width).expect("hostile range is bounded");
    bytes
        .get_mut(offset..end)
        .expect("hostile range is in bounds")
        .fill(0);
}

fn copy_range<const N: usize>(output: &mut [u8; N], offset: usize, source: &[u8]) {
    let end = offset
        .checked_add(source.len())
        .expect("fixture range is bounded");
    output
        .get_mut(offset..end)
        .expect("fixture range is in bounds")
        .copy_from_slice(source);
}

fn immutable_release(
    program_seed: u8,
    programdata_seed: u8,
    semantic_seed: u8,
) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        program(program_seed),
        program(200),
        bytes(programdata_seed),
        content(semantic_seed),
        bytes(semantic_seed.wrapping_add(40)),
        u64::from(semantic_seed) * 100,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("valid immutable release")
}

#[derive(Clone, Copy)]
struct ObservationParts {
    program: [u8; 32],
    program_owner: [u8; 32],
    program_executable: bool,
    programdata: [u8; 32],
    programdata_owner: [u8; 32],
    programdata_executable: bool,
    programdata_link: [u8; 32],
    loader_program: [u8; 32],
    deployment_slot: u64,
    elf_digest: [u8; 32],
    upgrade_authority: Option<[u8; 32]>,
}

impl ObservationParts {
    fn valid(release: ArtifactReleaseV1) -> Self {
        Self {
            program: release.program().to_bytes(),
            program_owner: release.loader_program().to_bytes(),
            program_executable: true,
            programdata: release.programdata(),
            programdata_owner: release.loader_program().to_bytes(),
            programdata_executable: false,
            programdata_link: release.programdata(),
            loader_program: release.loader_program().to_bytes(),
            deployment_slot: release.deployment_slot(),
            elf_digest: release.elf_digest(),
            upgrade_authority: release.upgrade_authority(),
        }
    }

    fn build(self) -> DeploymentObservationV1 {
        DeploymentObservationV1::new(
            self.program,
            self.program_owner,
            self.program_executable,
            self.programdata,
            self.programdata_owner,
            self.programdata_executable,
            self.programdata_link,
            self.loader_program,
            self.deployment_slot,
            self.elf_digest,
            self.upgrade_authority,
        )
        .expect("nonzero observation coordinates")
    }
}

#[derive(Clone, Copy)]
struct Fixture {
    release_set_id: ContentId,
    release_set: ExecutionReleaseSetV1,
    artifact_ids: [ArtifactReleaseIdV1; 5],
    releases: [ArtifactReleaseV1; 5],
}

impl Fixture {
    fn distinct() -> Self {
        let artifact_ids = [
            artifact_id(21),
            artifact_id(22),
            artifact_id(23),
            artifact_id(24),
            artifact_id(25),
        ];
        let releases = [
            immutable_release(1, 51, 61),
            immutable_release(2, 52, 62),
            immutable_release(3, 53, 63),
            immutable_release(4, 54, 64),
            immutable_release(5, 55, 65),
        ];
        let release_set = release_set(artifact_ids, releases);
        Self {
            release_set_id: content(91),
            release_set,
            artifact_ids,
            releases,
        }
    }

    fn inputs(self) -> ExecutionReleaseActivationInputsV1 {
        activation_inputs(self.artifact_ids, self.releases)
    }

    fn activate(self) -> ActivatedExecutionReleaseSetV1 {
        activate_execution_release_set_v1(self.release_set_id, &self.release_set, &self.inputs())
            .expect("valid activation")
    }
}

fn binding(
    artifact_ids: [ArtifactReleaseIdV1; 5],
    releases: [ArtifactReleaseV1; 5],
    index: usize,
) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(
        copied(&releases, index).program(),
        copied(&artifact_ids, index),
    )
}

fn release_set(
    artifact_ids: [ArtifactReleaseIdV1; 5],
    releases: [ArtifactReleaseV1; 5],
) -> ExecutionReleaseSetV1 {
    ExecutionReleaseSetV1::new(
        binding(artifact_ids, releases, 0),
        binding(artifact_ids, releases, 1),
        binding(artifact_ids, releases, 2),
        binding(artifact_ids, releases, 3),
        binding(artifact_ids, releases, 4),
    )
    .expect("consistent release-set bindings")
}

fn activation_input(
    artifact_release_id: ArtifactReleaseIdV1,
    release: ArtifactReleaseV1,
) -> ArtifactActivationInputV1 {
    ArtifactActivationInputV1::new(
        artifact_release_id,
        release,
        ObservationParts::valid(release).build(),
    )
}

fn activation_inputs(
    artifact_ids: [ArtifactReleaseIdV1; 5],
    releases: [ArtifactReleaseV1; 5],
) -> ExecutionReleaseActivationInputsV1 {
    ExecutionReleaseActivationInputsV1::new(
        activation_input(artifact_ids[0], releases[0]),
        activation_input(artifact_ids[1], releases[1]),
        activation_input(artifact_ids[2], releases[2]),
        activation_input(artifact_ids[3], releases[3]),
        activation_input(artifact_ids[4], releases[4]),
    )
}

#[test]
fn artifact_release_has_one_exact_canonical_encoding() {
    let immutable = immutable_release(1, 51, 61);
    let encoded = immutable.to_bytes();
    assert_eq!(encoded.len(), ARTIFACT_RELEASE_BYTES_V1);
    assert_eq!(&encoded[..8], &ARTIFACT_RELEASE_MAGIC_V1);
    assert_eq!(&encoded[16..48], immutable.program().as_bytes());
    assert_eq!(&encoded[48..80], immutable.loader_program().as_bytes());
    assert_eq!(&encoded[80..112], &immutable.programdata());
    assert_eq!(
        &encoded[112..144],
        immutable.semantic_release_id().as_bytes()
    );
    assert_eq!(&encoded[144..176], &immutable.elf_digest());
    assert_eq!(
        &encoded[176..184],
        &immutable.deployment_slot().to_le_bytes()
    );
    assert_eq!(&encoded[184..216], &[0; 32]);
    assert_eq!(ArtifactReleaseV1::decode(&encoded), Ok(immutable));

    let authority = bytes(77);
    let upgradeable = ArtifactReleaseV1::new(
        program(6),
        program(200),
        bytes(56),
        content(66),
        bytes(106),
        700,
        ArtifactUpgradePolicyV1::ExactAuthority,
        Some(authority),
    )
    .expect("valid exact authority release");
    assert_eq!(upgradeable.to_bytes()[12], 1);
    assert_eq!(&upgradeable.to_bytes()[184..216], &authority);
    assert_eq!(
        ArtifactReleaseV1::decode(&upgradeable.to_bytes()),
        Ok(upgradeable)
    );
}

#[test]
fn artifact_release_decoder_refuses_malformed_or_noncanonical_bytes() {
    let release = immutable_release(1, 51, 61);
    let encoded = release.to_bytes();
    assert_eq!(
        ArtifactReleaseV1::decode(&encoded[..215]),
        Err(Error::InvalidLength)
    );
    let mut extended = encoded.to_vec();
    extended.push(0);
    assert_eq!(
        ArtifactReleaseV1::decode(&extended),
        Err(Error::InvalidLength)
    );

    for (offset, expected) in [
        (0, Error::InvalidMagic),
        (8, Error::UnsupportedSchema),
        (10, Error::UnsupportedArtifactProfile),
        (13, Error::NonCanonicalReservedBytes),
        (80, Error::ZeroIdentity),
        (112, Error::ZeroIdentity),
        (144, Error::ZeroIdentity),
    ] {
        let mut hostile = encoded;
        if matches!(offset, 80 | 112 | 144) {
            zero_range(&mut hostile, offset, 32);
        } else {
            flip_at(&mut hostile, offset);
        }
        assert_eq!(ArtifactReleaseV1::decode(&hostile), Err(expected));
    }

    let mut authority_on_immutable = encoded;
    authority_on_immutable[184] = 1;
    assert_eq!(
        ArtifactReleaseV1::decode(&authority_on_immutable),
        Err(Error::NonCanonicalUpgradeAuthority)
    );
    let mut unsupported_policy = encoded;
    unsupported_policy[12] = 2;
    assert_eq!(
        ArtifactReleaseV1::decode(&unsupported_policy),
        Err(Error::NonCanonicalUpgradeAuthority)
    );
    let mut missing_exact_authority = encoded;
    missing_exact_authority[12] = 1;
    assert_eq!(
        ArtifactReleaseV1::decode(&missing_exact_authority),
        Err(Error::NonCanonicalUpgradeAuthority)
    );
}

#[test]
fn artifact_release_constructor_refuses_aliases_and_noncanonical_upgrade_policies() {
    let base = immutable_release(1, 51, 61);
    let create = |program_id, loader_id, programdata, policy, authority| {
        ArtifactReleaseV1::new(
            program_id,
            loader_id,
            programdata,
            content(61),
            bytes(101),
            6_100,
            policy,
            authority,
        )
    };
    assert_eq!(
        create(
            program(1),
            program(1),
            bytes(51),
            ArtifactUpgradePolicyV1::Immutable,
            None
        ),
        Err(Error::AliasedLoaderIdentity)
    );
    assert_eq!(
        create(
            program(1),
            program(200),
            program(1).to_bytes(),
            ArtifactUpgradePolicyV1::Immutable,
            None
        ),
        Err(Error::AliasedLoaderIdentity)
    );
    assert_eq!(
        create(
            program(1),
            program(200),
            program(200).to_bytes(),
            ArtifactUpgradePolicyV1::Immutable,
            None
        ),
        Err(Error::AliasedLoaderIdentity)
    );
    assert_eq!(
        create(
            program(1),
            program(200),
            bytes(51),
            ArtifactUpgradePolicyV1::Immutable,
            Some(bytes(77)),
        ),
        Err(Error::NonCanonicalUpgradeAuthority)
    );
    assert_eq!(
        create(
            program(1),
            program(200),
            bytes(51),
            ArtifactUpgradePolicyV1::ExactAuthority,
            None,
        ),
        Err(Error::NonCanonicalUpgradeAuthority)
    );
    assert_eq!(base.upgrade_policy(), ArtifactUpgradePolicyV1::Immutable);
}

#[test]
fn deployment_authentication_refuses_every_substitution_dimension() {
    let release = immutable_release(1, 51, 61);
    let valid = ObservationParts::valid(release);
    release
        .authenticate_deployment(valid.build())
        .expect("valid observation authenticates");

    let cases = [
        (
            ObservationParts {
                program: bytes(9),
                ..valid
            },
            Error::DeploymentIdentityMismatch,
        ),
        (
            ObservationParts {
                programdata: bytes(9),
                ..valid
            },
            Error::DeploymentIdentityMismatch,
        ),
        (
            ObservationParts {
                loader_program: bytes(9),
                ..valid
            },
            Error::DeploymentIdentityMismatch,
        ),
        (
            ObservationParts {
                programdata_link: bytes(9),
                ..valid
            },
            Error::ProgramDataLinkMismatch,
        ),
        (
            ObservationParts {
                program_owner: bytes(9),
                ..valid
            },
            Error::LoaderOwnerMismatch,
        ),
        (
            ObservationParts {
                programdata_owner: bytes(9),
                ..valid
            },
            Error::LoaderOwnerMismatch,
        ),
        (
            ObservationParts {
                program_executable: false,
                ..valid
            },
            Error::ProgramNotExecutable,
        ),
        (
            ObservationParts {
                programdata_executable: true,
                ..valid
            },
            Error::ProgramDataExecutable,
        ),
        (
            ObservationParts {
                deployment_slot: valid.deployment_slot + 1,
                ..valid
            },
            Error::DeploymentSlotMismatch,
        ),
        (
            ObservationParts {
                elf_digest: bytes(9),
                ..valid
            },
            Error::ElfDigestMismatch,
        ),
        (
            ObservationParts {
                upgrade_authority: Some(bytes(9)),
                ..valid
            },
            Error::UpgradeAuthorityMismatch,
        ),
    ];
    for (hostile, expected) in cases {
        assert_eq!(
            release.authenticate_deployment(hostile.build()),
            Err(expected)
        );
    }
}

#[test]
fn activation_closes_every_role_and_roundtrips_its_projection() {
    let fixture = Fixture::distinct();
    let activated = fixture.activate();
    assert_eq!(ACTIVATION_PDA_DOMAIN_V1, b"dclutch:release-activation:v1");
    assert_eq!(activated.execution_release_set_id(), fixture.release_set_id);
    assert_eq!(activated.release_set_projection(), Ok(fixture.release_set));
    for (role, index) in [
        (ExecutionRoleV1::Core, 0),
        (ExecutionRoleV1::Claims, 1),
        (ExecutionRoleV1::Trading, 2),
        (ExecutionRoleV1::Resolution, 3),
        (ExecutionRoleV1::Custody, 4),
    ] {
        let activated_role = activated.role(role);
        assert_eq!(
            activated_role.artifact_release_id(),
            copied(&fixture.artifact_ids, index)
        );
        assert_eq!(activated_role.release(), copied(&fixture.releases, index));
    }
    let encoded = activated.to_bytes();
    assert_eq!(encoded.len(), ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1);
    assert_eq!(&encoded[..8], &ACTIVATED_EXECUTION_RELEASE_SET_MAGIC_V1);

    let mut expected = [0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    copy_range(&mut expected, 0, &ACTIVATED_EXECUTION_RELEASE_SET_MAGIC_V1);
    copy_range(
        &mut expected,
        8,
        &ACTIVATED_EXECUTION_RELEASE_SET_SCHEMA_VERSION_V1.to_le_bytes(),
    );
    copy_range(
        &mut expected,
        10,
        &ACTIVATED_EXECUTION_RELEASE_SET_PROFILE_V1.to_le_bytes(),
    );
    copy_range(&mut expected, 16, fixture.release_set_id.as_bytes());
    for index in 0..5 {
        let role_offset = ROLE_CACHE_HEADER_BYTES + index * ROLE_CACHE_BYTES;
        copy_range(
            &mut expected,
            role_offset,
            copied(&fixture.artifact_ids, index).as_bytes(),
        );
        copy_range(
            &mut expected,
            role_offset + 32,
            &copied(&fixture.releases, index).to_bytes(),
        );
    }
    assert_eq!(encoded, expected);
    assert_eq!(
        ActivatedExecutionReleaseSetV1::decode(&encoded),
        Ok(activated)
    );
}

#[test]
fn streaming_activation_is_byte_identical_to_the_owned_host_api() {
    let fixture = Fixture::distinct();
    let expected = fixture.activate().to_bytes();
    let mut streamed = [0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut streamed, fixture.release_set_id)
        .expect("initialize exact cache");
    for (role, index) in [
        (ExecutionRoleV1::Core, 0),
        (ExecutionRoleV1::Claims, 1),
        (ExecutionRoleV1::Trading, 2),
        (ExecutionRoleV1::Resolution, 3),
        (ExecutionRoleV1::Custody, 4),
    ] {
        let input = activation_input(
            copied(&fixture.artifact_ids, index),
            copied(&fixture.releases, index),
        );
        activate_execution_role_into_v1(
            &mut streamed,
            fixture.release_set_id,
            &fixture.release_set,
            role,
            &input,
        )
        .expect("stream one authenticated role");
    }
    assert_eq!(streamed, expected);
    let view = crate::ActivatedExecutionReleaseSetViewV1::decode(&streamed)
        .expect("complete borrowed view");
    assert_eq!(view.execution_release_set_id(), Ok(fixture.release_set_id));
    assert_eq!(view.release_set_projection(), Ok(fixture.release_set));
}

/// A cache the Registry actually wrote reports its own progress.
///
/// The regression this pins is one byte wide and it stopped every local
/// founding in the tree. `ACTIVATION_CACHE_BUMP_OFFSET_V1` sits at 12, the
/// first of what used to be four reserved bytes, and the selection comparison
/// spanned `0..48` — so it compared the bump against
/// `ActivatedExecutionReleaseSetV1::to_bytes`, a projection of the RELEASE SET
/// that has no field for a fact about an ACCOUNT ADDRESS and leaves that byte
/// zero. `put_activation_cache_bump_v1` refuses to write zero. The two are
/// therefore unequal for every cache the Registry has ever signed into
/// existence, and `ReleaseSetSelectionMismatch` was unreachable-by-success:
/// role activation succeeded five times and the first read of the resulting
/// cache refused.
///
/// The bump is a carrier, not a canonicality field — this file already says so
/// where the decoder tolerates both a present and an absent bump. So a cache
/// carrying ANY bump must report the same progress as one carrying none, and a
/// real selection difference must still refuse.
#[test]
fn a_cache_carrying_its_own_bump_still_reports_its_progress() {
    let fixture = Fixture::distinct();
    let expected = fixture.activate();
    let mut cache = [0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut cache, fixture.release_set_id)
        .expect("initialize exact cache");

    // Nothing written yet, with and without the bump the Registry records.
    let empty = activation_cache_progress_v1(&cache, expected).expect("vacant progress");
    assert_eq!(empty.written_count(), 0);
    put_activation_cache_bump_v1(&mut cache, 254).expect("record the bump");
    let empty_with_bump =
        activation_cache_progress_v1(&cache, expected).expect("vacant progress, bump carried");
    assert_eq!(empty_with_bump.written_count(), 0);

    // Then every role, one transaction at a time, exactly as the Registry does.
    for (role, index) in [
        (ExecutionRoleV1::Core, 0),
        (ExecutionRoleV1::Claims, 1),
        (ExecutionRoleV1::Trading, 2),
        (ExecutionRoleV1::Resolution, 3),
        (ExecutionRoleV1::Custody, 4),
    ] {
        activate_execution_role_into_v1(
            &mut cache,
            fixture.release_set_id,
            &fixture.release_set,
            role,
            &activation_input(
                copied(&fixture.artifact_ids, index),
                copied(&fixture.releases, index),
            ),
        )
        .expect("stream one authenticated role");
        let progress = activation_cache_progress_v1(&cache, expected)
            .expect("a Registry-written cache reports its own progress");
        assert!(progress.is_written(role));
        assert_eq!(progress.written_count(), index + 1);
    }
    assert!(
        activation_cache_progress_v1(&cache, expected)
            .expect("complete progress")
            .is_complete()
    );

    // Every bump a derivation can produce behaves the same. 0 is not one of
    // them, and is the "written before this byte existed" body.
    for bump in [1_u8, 128, 255] {
        let mut carried = cache;
        put_activation_cache_bump_v1(&mut carried, bump).expect("record another bump");
        assert!(
            activation_cache_progress_v1(&carried, expected)
                .expect("progress does not depend on which bump the address has")
                .is_complete()
        );
    }

    // And the check the span still owns: a projection naming another release
    // set refuses against this cache, bump carried or not.
    let mut foreign = Fixture::distinct();
    foreign.release_set_id = content(92);
    assert_eq!(
        activation_cache_progress_v1(&cache, foreign.activate()),
        Err(Error::ReleaseSetSelectionMismatch)
    );
}

#[test]
fn finalized_release_set_is_the_only_core_binding() {
    let fixture = Fixture::distinct();
    let activated = activate_execution_release_set_v1(
        fixture.release_set_id,
        &fixture.release_set,
        &fixture.inputs(),
    )
    .expect("the authenticated Core artifact needs no Registry identity input");
    assert_eq!(
        activated.role(ExecutionRoleV1::Core).release().program(),
        fixture.release_set.binding(ExecutionRoleV1::Core).program()
    );

    let mut substituted_ids = fixture.artifact_ids;
    substituted_ids[2] = artifact_id(99);
    assert_eq!(
        activate_execution_release_set_v1(
            fixture.release_set_id,
            &fixture.release_set,
            &activation_inputs(substituted_ids, fixture.releases),
        ),
        Err(Error::RoleArtifactReleaseMismatch)
    );

    let mut substituted_releases = fixture.releases;
    substituted_releases[3] = immutable_release(99, 59, 69);
    assert_eq!(
        activate_execution_release_set_v1(
            fixture.release_set_id,
            &fixture.release_set,
            &activation_inputs(fixture.artifact_ids, substituted_releases),
        ),
        Err(Error::RoleProgramMismatch)
    );
}

#[test]
fn activation_refuses_stale_slot_and_reauthentication_catches_later_upgrade() {
    let fixture = Fixture::distinct();
    let stale_release = fixture.releases[4];
    let valid = ObservationParts::valid(stale_release);
    let stale = ObservationParts {
        deployment_slot: valid.deployment_slot + 1,
        ..valid
    }
    .build();
    let inputs = ExecutionReleaseActivationInputsV1::new(
        activation_input(fixture.artifact_ids[0], fixture.releases[0]),
        activation_input(fixture.artifact_ids[1], fixture.releases[1]),
        activation_input(fixture.artifact_ids[2], fixture.releases[2]),
        activation_input(fixture.artifact_ids[3], fixture.releases[3]),
        ArtifactActivationInputV1::new(fixture.artifact_ids[4], stale_release, stale),
    );
    assert_eq!(
        activate_execution_release_set_v1(fixture.release_set_id, &fixture.release_set, &inputs,),
        Err(Error::DeploymentSlotMismatch)
    );

    let activated = fixture.activate();
    assert_eq!(
        activated
            .role(ExecutionRoleV1::Custody)
            .authenticate_current_deployment(stale),
        Err(Error::DeploymentSlotMismatch)
    );
}

#[test]
fn aliased_roles_must_share_one_complete_activation() {
    let fixture = Fixture::distinct();
    let mut artifact_ids = fixture.artifact_ids;
    let mut releases = fixture.releases;
    artifact_ids[2] = artifact_ids[1];
    releases[2] = releases[1];
    let aliased_set = release_set(artifact_ids, releases);
    let aliased_inputs = activation_inputs(artifact_ids, releases);
    let activated =
        activate_execution_release_set_v1(fixture.release_set_id, &aliased_set, &aliased_inputs)
            .expect("identical alias activates");
    assert_eq!(
        activated.role(ExecutionRoleV1::Claims),
        activated.role(ExecutionRoleV1::Trading)
    );

    let claims_release = releases[1];
    let valid = ObservationParts::valid(claims_release);
    let stale = ObservationParts {
        deployment_slot: valid.deployment_slot + 1,
        ..valid
    }
    .build();
    let hostile_inputs = ExecutionReleaseActivationInputsV1::new(
        activation_input(artifact_ids[0], releases[0]),
        activation_input(artifact_ids[1], releases[1]),
        ArtifactActivationInputV1::new(artifact_ids[2], releases[2], stale),
        activation_input(artifact_ids[3], releases[3]),
        activation_input(artifact_ids[4], releases[4]),
    );
    assert_eq!(
        activate_execution_release_set_v1(fixture.release_set_id, &aliased_set, &hostile_inputs,),
        Err(Error::AliasedRoleActivationMismatch)
    );

    let mut hostile_cache = activated.to_bytes();
    let trading_slot_offset =
        ROLE_CACHE_HEADER_BYTES + 2 * ROLE_CACHE_BYTES + 32 + ARTIFACT_DEPLOYMENT_SLOT_OFFSET;
    let slot_byte = hostile_cache
        .get_mut(trading_slot_offset)
        .expect("trading deployment slot is in bounds");
    *slot_byte ^= 1;
    assert_eq!(
        ActivatedExecutionReleaseSetV1::decode(&hostile_cache),
        Err(Error::AliasedRoleActivationMismatch)
    );
}

/// The BORROWED view is the decoder every adapter actually runs, and its
/// aliasing scan is a separate implementation from the owned decoder's.
///
/// `aliased_roles_must_share_one_complete_activation` above has always pinned
/// the owned `ActivatedExecutionReleaseSetV1::decode`. Nothing pinned the view,
/// whose scan reads the same pairs out of borrowed bytes -- so this states the
/// same accusation against the code that Trading, Claims, Custody, Core and the
/// Registry's own `Reauthenticate` handler all reach.
///
/// Both arms are here because the refusal is only attributable with the accept
/// beside it: an aliased pair that agrees in every field is a LEGAL cache and
/// must decode, and only the one flipped deployment-slot byte may refuse.
#[test]
fn the_borrowed_view_refuses_an_aliased_role_that_disagrees_in_one_byte() {
    let fixture = Fixture::distinct();
    let mut artifact_ids = fixture.artifact_ids;
    let mut releases = fixture.releases;
    artifact_ids[2] = artifact_ids[1];
    releases[2] = releases[1];
    let aliased_set = release_set(artifact_ids, releases);
    let aliased_inputs = activation_inputs(artifact_ids, releases);
    let activated =
        activate_execution_release_set_v1(fixture.release_set_id, &aliased_set, &aliased_inputs)
            .expect("identical alias activates");

    let legal_cache = activated.to_bytes();
    let view = ActivatedExecutionReleaseSetViewV1::decode(&legal_cache)
        .expect("an aliased pair that agrees in every field is a legal cache");
    assert_eq!(
        view.role(ExecutionRoleV1::Claims).expect("claims decodes"),
        view.role(ExecutionRoleV1::Trading)
            .expect("trading decodes"),
    );

    let mut hostile_cache = legal_cache;
    let trading_slot_offset =
        ROLE_CACHE_HEADER_BYTES + 2 * ROLE_CACHE_BYTES + 32 + ARTIFACT_DEPLOYMENT_SLOT_OFFSET;
    let slot_byte = hostile_cache
        .get_mut(trading_slot_offset)
        .expect("trading deployment slot is in bounds");
    *slot_byte ^= 1;
    assert_eq!(
        ActivatedExecutionReleaseSetViewV1::decode(&hostile_cache).map(|_| ()),
        Err(Error::AliasedRoleActivationMismatch)
    );
}

#[test]
fn activation_cache_decoder_refuses_malformed_headers_and_role_identities() {
    let encoded = Fixture::distinct().activate().to_bytes();
    let truncated = encoded
        .get(..encoded.len() - 1)
        .expect("truncated cache slice is in bounds");
    assert_eq!(
        ActivatedExecutionReleaseSetV1::decode(truncated),
        Err(Error::InvalidLength)
    );
    let mut extended = encoded.to_vec();
    extended.push(0);
    assert_eq!(
        ActivatedExecutionReleaseSetV1::decode(&extended),
        Err(Error::InvalidLength)
    );
    for (offset, expected) in [
        (0, Error::InvalidMagic),
        (8, Error::UnsupportedSchema),
        (10, Error::UnsupportedArtifactProfile),
        // 12 is the cache's own PDA bump and is deliberately NOT zero-checked;
        // see the tolerated-bump assertions below. 13..16 are what remains of
        // the reserved field.
        (13, Error::NonCanonicalReservedBytes),
        (15, Error::NonCanonicalReservedBytes),
        (16, Error::ZeroIdentity),
    ] {
        let mut hostile = encoded;
        if offset == 16 {
            zero_range(&mut hostile, 16, 32);
        } else {
            flip_at(&mut hostile, offset);
        }
        assert_eq!(
            ActivatedExecutionReleaseSetV1::decode(&hostile),
            Err(expected)
        );
    }

    // The bump byte is a carrier, not a canonicality field: a body that has one
    // decodes and reports it, and a body written before it existed decodes and
    // reports `None` so its readers fall back to the search. Both are valid.
    let mut carried = encoded.to_vec();
    assert_eq!(
        ActivatedExecutionReleaseSetViewV1::decode(&carried)
            .expect("cache without a bump")
            .cache_bump(),
        Ok(None)
    );
    put_activation_cache_bump_v1(&mut carried, 254).expect("record the bump");
    assert_eq!(
        ActivatedExecutionReleaseSetViewV1::decode(&carried)
            .expect("cache carrying a bump")
            .cache_bump(),
        Ok(Some(254))
    );
    assert_eq!(
        put_activation_cache_bump_v1(&mut carried, 0),
        Err(Error::NonCanonicalReservedBytes),
        "zero is not a bump any derivation produces, so it is refused at the writer"
    );

    let mut zero_artifact_id = encoded;
    zero_artifact_id[ROLE_CACHE_HEADER_BYTES..ROLE_CACHE_HEADER_BYTES + 32].fill(0);
    assert_eq!(
        ActivatedExecutionReleaseSetV1::decode(&zero_artifact_id),
        Err(Error::ReleaseSet(
            crate::release_set::Error::ZeroArtifactReleaseId
        ))
    );
}

// ---------------------------------------------------------------------------
// Release-set lineage
// ---------------------------------------------------------------------------

const LINEAGE_MOVED_ROLES_OFFSET: usize = 80;
const LINEAGE_AUTHORITIES_OFFSET: usize = 88;

fn lineage_consent(
    moved: [bool; EXECUTION_ROLE_COUNT_V1],
) -> [Option<[u8; 32]>; EXECUTION_ROLE_COUNT_V1] {
    let mut consent = [None; EXECUTION_ROLE_COUNT_V1];
    for role in EXECUTION_ROLE_ORDER_V1 {
        let index = role.role_index();
        if let Some(slot) = consent.get_mut(index).filter(|_| copied(&moved, index)) {
            // A distinct authority per role, so a test that swapped two slots
            // would be caught instead of comparing equal.
            *slot = Some(bytes(
                0xa0 + u8::try_from(index).expect("role index is small"),
            ));
        }
    }
    consent
}

fn lineage_fixture() -> ReleaseLineageV1 {
    // Core and Trading moved; Claims, Resolution and Custody did not.
    ReleaseLineageV1::new(
        content(0x11),
        content(0x22),
        lineage_consent([true, false, true, false, false]),
    )
    .expect("canonical lineage fixture")
}

#[test]
fn lineage_layout_is_the_wire_contract() {
    assert_eq!(RELEASE_LINEAGE_BYTES_V1, 248);
    assert_eq!(RELEASE_LINEAGE_MAGIC_V1, *b"DCLTRLN1");
    assert_eq!(RELEASE_LINEAGE_PDA_DOMAIN_V1.len(), 26);
    assert_eq!(RELEASE_LINEAGE_PDA_SEED_COUNT_V1, 2);
    assert_eq!(
        RELEASE_LINEAGE_PDA_DOMAIN_V1.as_slice(),
        b"dclutch:release-lineage:v1".as_slice()
    );

    let encoded = lineage_fixture().to_bytes();
    assert_eq!(encoded.len(), RELEASE_LINEAGE_BYTES_V1);
    assert_eq!(
        encoded.get(..8).expect("magic is in bounds"),
        RELEASE_LINEAGE_MAGIC_V1.as_slice()
    );
    assert_eq!(
        encoded.get(8..10).expect("schema is in bounds"),
        RELEASE_LINEAGE_SCHEMA_VERSION_V1.to_le_bytes().as_slice()
    );
    assert_eq!(
        encoded.get(10..12).expect("profile is in bounds"),
        RELEASE_LINEAGE_PROFILE_V1.to_le_bytes().as_slice()
    );
    assert!(
        encoded
            .get(12..16)
            .expect("header reserve is in bounds")
            .iter()
            .all(|byte| *byte == 0)
    );
    assert_eq!(
        encoded.get(16..48).expect("predecessor is in bounds"),
        bytes(0x11).as_slice()
    );
    assert_eq!(
        encoded.get(48..80).expect("successor is in bounds"),
        bytes(0x22).as_slice()
    );
    // Core and Trading moved; the other three did not, and the three bytes
    // after the mask are reserve, not a sixth role.
    assert_eq!(
        encoded.get(80..85).expect("moved mask is in bounds"),
        [1_u8, 0, 1, 0, 0].as_slice()
    );
    assert!(
        encoded
            .get(85..88)
            .expect("mask reserve is in bounds")
            .iter()
            .all(|byte| *byte == 0)
    );
    assert_eq!(
        encoded.get(88..120).expect("Core authority is in bounds"),
        bytes(0xa0).as_slice()
    );
    assert!(
        encoded
            .get(120..152)
            .expect("Claims authority is in bounds")
            .iter()
            .all(|byte| *byte == 0)
    );
    assert_eq!(
        encoded
            .get(152..184)
            .expect("Trading authority is in bounds"),
        bytes(0xa2).as_slice()
    );
}

#[test]
fn lineage_roundtrips_and_reads_back_per_role() {
    let lineage = lineage_fixture();
    let decoded = ReleaseLineageV1::decode(&lineage.to_bytes()).expect("canonical lineage decodes");
    assert_eq!(decoded, lineage);
    assert_eq!(decoded.predecessor(), content(0x11));
    assert_eq!(decoded.successor(), content(0x22));
    assert_eq!(decoded.to_bytes(), lineage.to_bytes());

    for (role, moved) in [
        (ExecutionRoleV1::Core, true),
        (ExecutionRoleV1::Claims, false),
        (ExecutionRoleV1::Trading, true),
        (ExecutionRoleV1::Resolution, false),
        (ExecutionRoleV1::Custody, false),
    ] {
        assert_eq!(decoded.moved(role), moved, "{role:?} moved verdict");
        assert_eq!(
            decoded.consenting_authority(role).is_some(),
            moved,
            "{role:?} consent presence follows its moved verdict"
        );
    }
    assert_eq!(
        decoded.consenting_authority(ExecutionRoleV1::Core),
        Some(bytes(0xa0))
    );
    assert_eq!(
        decoded.consenting_authority(ExecutionRoleV1::Trading),
        Some(bytes(0xa2))
    );
    assert_eq!(decoded.consenting_authority(ExecutionRoleV1::Claims), None);
}

#[test]
fn lineage_header_hostiles_each_refuse_at_their_own_field() {
    let encoded = lineage_fixture().to_bytes();
    assert!(ReleaseLineageV1::decode(&encoded).is_ok());

    for (offset, expected) in [
        (0, Error::InvalidMagic),
        (8, Error::UnsupportedSchema),
        (10, Error::UnsupportedArtifactProfile),
        (12, Error::NonCanonicalReservedBytes),
        (85, Error::NonCanonicalReservedBytes),
    ] {
        let mut hostile = encoded;
        flip_at(&mut hostile, offset);
        assert_eq!(
            ReleaseLineageV1::decode(&hostile),
            Err(expected),
            "flipping byte {offset} must refuse as {expected:?}"
        );
    }

    let truncated = encoded
        .get(..encoded.len() - 1)
        .expect("truncated lineage slice is in bounds");
    assert_eq!(
        ReleaseLineageV1::decode(truncated),
        Err(Error::InvalidLength)
    );
    let mut extended = encoded.to_vec();
    extended.push(0);
    assert_eq!(
        ReleaseLineageV1::decode(&extended),
        Err(Error::InvalidLength)
    );
    assert_eq!(ReleaseLineageV1::decode(&[]), Err(Error::InvalidLength));
}

#[test]
fn lineage_refuses_a_zero_endpoint() {
    for offset in [16, 48] {
        let mut hostile = lineage_fixture().to_bytes();
        zero_range(&mut hostile, offset, 32);
        assert_eq!(
            ReleaseLineageV1::decode(&hostile),
            Err(Error::ZeroIdentity),
            "a zero endpoint at {offset} is not an identity"
        );
    }
}

#[test]
fn lineage_refuses_self_succession() {
    assert_eq!(
        ReleaseLineageV1::new(
            content(0x11),
            content(0x11),
            lineage_consent([true, false, false, false, false]),
        ),
        Err(Error::LineageSelfSuccession)
    );

    // And on the wire, where the constructor was never called.
    let mut hostile = lineage_fixture().to_bytes();
    let predecessor = bytes(0x11);
    hostile
        .get_mut(48..80)
        .expect("successor is in bounds")
        .copy_from_slice(&predecessor);
    assert_eq!(
        ReleaseLineageV1::decode(&hostile),
        Err(Error::LineageSelfSuccession)
    );
}

#[test]
fn lineage_refuses_a_hop_that_moved_nothing() {
    assert_eq!(
        ReleaseLineageV1::new(
            content(0x11),
            content(0x22),
            [None; EXECUTION_ROLE_COUNT_V1]
        ),
        Err(Error::LineageWithoutMovedRole)
    );

    let mut hostile = lineage_fixture().to_bytes();
    zero_range(&mut hostile, LINEAGE_MOVED_ROLES_OFFSET, 5);
    zero_range(
        &mut hostile,
        LINEAGE_AUTHORITIES_OFFSET,
        EXECUTION_ROLE_COUNT_V1 * 32,
    );
    assert_eq!(
        ReleaseLineageV1::decode(&hostile),
        Err(Error::LineageWithoutMovedRole)
    );
}

#[test]
fn lineage_refuses_consent_that_disagrees_with_its_mask() {
    // A role claimed as moved whose consent slot is empty: the forgery H2
    // describes, seen by a reader who holds only the record.
    let mut moved_without_authority = lineage_fixture().to_bytes();
    zero_range(&mut moved_without_authority, LINEAGE_AUTHORITIES_OFFSET, 32);
    assert_eq!(
        ReleaseLineageV1::decode(&moved_without_authority),
        Err(Error::NonCanonicalLineageConsent)
    );

    // A key recorded for a role that did not move: consent nobody asked for.
    let mut authority_without_move = lineage_fixture().to_bytes();
    authority_without_move
        .get_mut(LINEAGE_AUTHORITIES_OFFSET + 32..LINEAGE_AUTHORITIES_OFFSET + 64)
        .expect("Claims authority is in bounds")
        .copy_from_slice(&bytes(0xbb));
    assert_eq!(
        ReleaseLineageV1::decode(&authority_without_move),
        Err(Error::NonCanonicalLineageConsent)
    );

    // The mask is a canonical boolean, not any nonzero byte.
    for hostile_byte in [2_u8, 0xff] {
        let mut hostile = lineage_fixture().to_bytes();
        if let Some(mask) = hostile.get_mut(LINEAGE_MOVED_ROLES_OFFSET + 1) {
            *mask = hostile_byte;
        }
        assert_eq!(
            ReleaseLineageV1::decode(&hostile),
            Err(Error::NonCanonicalLineageConsent),
            "mask byte {hostile_byte} is not a canonical moved flag"
        );
    }

    // And the constructor cannot be handed a zero key wearing `Some`.
    let mut consent = lineage_consent([true, false, false, false, false]);
    if let Some(slot) = consent.get_mut(0) {
        *slot = Some([0; 32]);
    }
    assert_eq!(
        ReleaseLineageV1::new(content(0x11), content(0x22), consent),
        Err(Error::NonCanonicalLineageConsent)
    );
}

#[test]
fn lineage_consent_is_indexed_by_the_canonical_role_order() {
    // The record's authority table is role-indexed, so a consumer that walked
    // it in any other order would read another role's consent. This pins the
    // order to its sole author rather than to a second copy of it here.
    let consent = lineage_consent([false, true, false, false, true]);
    let lineage =
        ReleaseLineageV1::new(content(0x33), content(0x44), consent).expect("lineage constructs");
    let encoded = lineage.to_bytes();
    for role in EXECUTION_ROLE_ORDER_V1 {
        let index = role.role_index();
        let slot = encoded
            .get(
                LINEAGE_AUTHORITIES_OFFSET + index * 32
                    ..LINEAGE_AUTHORITIES_OFFSET + (index + 1) * 32,
            )
            .expect("authority slot is in bounds");
        match lineage.consenting_authority(role) {
            Some(authority) => assert_eq!(slot, authority.as_slice(), "{role:?} authority slot"),
            None => assert!(
                slot.iter().all(|byte| *byte == 0),
                "{role:?} slot must stay zero"
            ),
        }
        assert_eq!(
            copied(
                encoded
                    .get(LINEAGE_MOVED_ROLES_OFFSET..LINEAGE_MOVED_ROLES_OFFSET + 5)
                    .and_then(|mask| <[u8; 5]>::try_from(mask).ok())
                    .as_ref()
                    .expect("mask is exactly five bytes"),
                index
            ) == 1,
            lineage.moved(role),
            "{role:?} mask byte must sit at its own role index"
        );
    }
}

// ---------------------------------------------------------------------------
// Release-set lineage: following the chain
// ---------------------------------------------------------------------------

/// One declared hop, paired with the set whose lineage address holds it.
fn lineage_hop(from: u8, to: u8) -> (ContentId, ReleaseLineageV1) {
    (
        content(from),
        ReleaseLineageV1::new(
            content(from),
            content(to),
            lineage_consent([true, false, true, false, false]),
        )
        .expect("canonical hop"),
    )
}

/// A lookup in the shape every real reader holds: keyed by derived address,
/// answering "undeclared" for every set nobody has superseded.
fn lineage_source(hops: &[(ContentId, ReleaseLineageV1)]) -> impl Fn(ContentId) -> LineageAt + '_ {
    move |sought| {
        hops.iter()
            .find(|(key, _)| *key == sought)
            .map_or(LineageAt::Undeclared, |(_, record)| {
                LineageAt::Declared(*record)
            })
    }
}

#[test]
fn a_chain_walks_forward_to_the_set_nobody_has_superseded() {
    // Three cuts: a market founded on 0x11 is two hops behind the world.
    let hops = [lineage_hop(0x11, 0x22), lineage_hop(0x22, 0x33)];
    let walk = walk_lineage_to_head(content(0x11), lineage_source(&hops))
        .expect("a complete chain reaches its head");

    assert_eq!(walk.endpoint(), content(0x33));
    assert_eq!(walk.hops(), 2, "one MigrateMarket per cut behind");
    assert!(!walk.is_already_current());
}

#[test]
fn a_market_already_on_the_head_walks_zero_hops_and_reads_as_already_current() {
    let hops = [lineage_hop(0x11, 0x22)];
    let walk = walk_lineage_to_head(content(0x22), lineage_source(&hops))
        .expect("the head of a chain is a complete walk");

    assert_eq!(walk.endpoint(), content(0x22));
    assert_eq!(walk.hops(), 0);
    // The lineage form of the deployment set's AlreadyCurrent disposition:
    // satisfied on an equality, with no receipt to produce.
    assert!(walk.is_already_current());

    let arrived = walk_lineage_to(content(0x22), content(0x22), lineage_source(&hops))
        .expect("an origin that is already the destination has arrived");
    assert!(arrived.is_already_current());
}

#[test]
fn walking_to_a_destination_stops_there_rather_than_running_on_to_the_head() {
    let hops = [lineage_hop(0x11, 0x22), lineage_hop(0x22, 0x33)];
    let walk = walk_lineage_to(content(0x11), content(0x22), lineage_source(&hops))
        .expect("the destination sits mid-chain");

    assert_eq!(walk.endpoint(), content(0x22));
    assert_eq!(walk.hops(), 1);
}

#[test]
fn a_gap_refuses_by_naming_the_set_that_still_owes_a_successor() {
    // The cut's real question: the world moved to 0x44, but the chain from the
    // traded market's founding set stops at 0x33. The refusal is the repair
    // instruction -- declare a successor FOR 0x33.
    let hops = [lineage_hop(0x11, 0x22), lineage_hop(0x22, 0x33)];

    assert_eq!(
        walk_lineage_to(content(0x11), content(0x44), lineage_source(&hops)),
        Err(LineageWalkRefusal::SuccessorUndeclared { at: content(0x33) })
    );

    // The same chain walked WITHOUT a destination is not a failure at all: an
    // undeclared successor is what ends a head-walk rather than what breaks it.
    assert_eq!(
        walk_lineage_to_head(content(0x11), lineage_source(&hops))
            .expect("a head-walk never refuses a gap")
            .endpoint(),
        content(0x33)
    );
}

#[test]
fn a_record_that_names_another_predecessor_is_evidence_about_nothing_here() {
    // Red-proof by mutation: take the canonical record and rewrite only its
    // predecessor run, then serve it at the address it no longer describes.
    let mut forged = lineage_fixture().to_bytes();
    let usurped = content(0x99).to_bytes();
    forged
        .get_mut(16..48)
        .expect("predecessor run")
        .copy_from_slice(usurped.as_slice());
    let forged = ReleaseLineageV1::decode(&forged).expect("the mutation is still a valid record");
    assert_eq!(forged.predecessor(), content(0x99));

    // Served under 0x11 -- the address a market on 0x11 derives.
    let hops = [(content(0x11), forged)];
    assert_eq!(
        walk_lineage_to_head(content(0x11), lineage_source(&hops)),
        Err(LineageWalkRefusal::Misaddressed {
            sought: content(0x11),
            found: content(0x99),
        })
    );
}

#[test]
fn an_undecodable_record_refuses_under_the_codecs_own_name() {
    let cause = Error::InvalidMagic;
    assert_eq!(
        walk_lineage_to_head(content(0x11), |_| LineageAt::Undecodable(cause)),
        Err(LineageWalkRefusal::Undecodable {
            at: content(0x11),
            cause,
        })
    );
    assert_eq!(
        Error::from(LineageWalkRefusal::Undecodable {
            at: content(0x11),
            cause,
        }),
        cause,
        "a decode refusal keeps its own name rather than gaining a second one"
    );
}

#[test]
fn a_cycle_terminates_on_the_hop_bound_instead_of_running_forever() {
    // DeclareSuccessor's forward-only conjunct makes this unbuildable on chain.
    // Against a hostile off-chain source it must still be bounded and named.
    let hops = [lineage_hop(0x11, 0x22), lineage_hop(0x22, 0x11)];
    let refusal = walk_lineage_to(content(0x11), content(0x44), lineage_source(&hops))
        .expect_err("a cycle never arrives");

    assert!(matches!(refusal, LineageWalkRefusal::TooLong { .. }));
    assert_eq!(Error::from(refusal), Error::LineageWalkTooLong);
}

#[test]
fn the_walk_refuses_one_hop_past_its_own_bound() {
    let hops: std::vec::Vec<_> = (0..=LINEAGE_WALK_MAX_HOPS_V1)
        .map(|step| lineage_hop(0x11 + step, 0x12 + step))
        .collect();
    let bound = usize::from(LINEAGE_WALK_MAX_HOPS_V1);

    // Exactly at the bound the walk still arrives.
    let reached = walk_lineage_to_head(content(0x11), lineage_source(&hops[..bound]))
        .expect("a chain of exactly the bound is walkable");
    assert_eq!(reached.hops(), LINEAGE_WALK_MAX_HOPS_V1);

    // One hop further it refuses, naming where it stopped.
    assert_eq!(
        walk_lineage_to_head(content(0x11), lineage_source(&hops)),
        Err(LineageWalkRefusal::TooLong {
            at: content(0x11 + LINEAGE_WALK_MAX_HOPS_V1),
        })
    );
}

#[test]
fn a_hop_authored_long_after_the_fact_is_byte_identical_to_a_timely_one() {
    // The honesty of retroactive lineage rests on an absence: the record has no
    // input that could express WHEN it was written, so a hop declared today for
    // two cohorts that superseded each other months ago is not a backdated
    // record -- it is the same record, and there is nothing in it to backdate.
    let endpoints = (content(0x11), content(0x22));
    let consent = lineage_consent([true, false, true, false, false]);

    let timely = ReleaseLineageV1::new(endpoints.0, endpoints.1, consent).expect("timely hop");
    let late = ReleaseLineageV1::new(endpoints.0, endpoints.1, consent).expect("late hop");

    assert_eq!(
        timely.to_bytes(),
        late.to_bytes(),
        "the encoding is a function of the endpoints and the consent, and of nothing else"
    );
    assert_eq!(timely, late);

    // Both reserved runs stay zero, so there is no spare room in the record
    // where a stamp could have been hidden and later disagreed with.
    let bytes = late.to_bytes();
    assert!(
        bytes
            .get(12..16)
            .expect("header reserved")
            .iter()
            .all(|b| *b == 0)
    );
    assert!(
        bytes
            .get(85..88)
            .expect("mask reserved")
            .iter()
            .all(|b| *b == 0)
    );

    // And a chain of such records walks, which is the whole deliverable: a
    // history recovered late is followable exactly like one recorded on time.
    let hops = [lineage_hop(0x11, 0x22), lineage_hop(0x22, 0x33)];
    assert_eq!(
        walk_lineage_to(content(0x11), content(0x33), lineage_source(&hops))
            .expect("a retroactively authored chain is a chain")
            .hops(),
        2
    );
}
