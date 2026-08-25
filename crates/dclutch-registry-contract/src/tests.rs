extern crate std;

use dclutch_core_contract::{ContentId, MarketIdentity};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1,
    ProgramIdentityV1,
};

use crate::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATED_EXECUTION_RELEASE_SET_MAGIC_V1,
    ACTIVATED_EXECUTION_RELEASE_SET_PROFILE_V1, ACTIVATED_EXECUTION_RELEASE_SET_SCHEMA_VERSION_V1,
    ACTIVATION_PDA_DOMAIN_V1, ARTIFACT_RELEASE_BYTES_V1, ARTIFACT_RELEASE_MAGIC_V1,
    ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, EXECUTION_AUTHORITY_MANIFEST_BYTES_V1,
    EXECUTION_AUTHORITY_MANIFEST_MAGIC_V1, Error, ExecutionAuthorityManifestV1,
    ExecutionReleaseActivationInputsV1, activate_execution_release_set_v1,
    authenticate_market_execution_v1,
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
    core_program: ProgramIdentityV1,
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
            core_program: releases[0].program(),
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
        activate_execution_release_set_v1(
            self.core_program,
            self.release_set_id,
            &self.release_set,
            &self.inputs(),
        )
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
fn activation_refuses_core_role_program_and_artifact_substitutions() {
    let fixture = Fixture::distinct();
    assert_eq!(
        activate_execution_release_set_v1(
            program(99),
            fixture.release_set_id,
            &fixture.release_set,
            &fixture.inputs(),
        ),
        Err(Error::CoreProgramMismatch)
    );

    let mut substituted_ids = fixture.artifact_ids;
    substituted_ids[2] = artifact_id(99);
    assert_eq!(
        activate_execution_release_set_v1(
            fixture.core_program,
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
            fixture.core_program,
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
        activate_execution_release_set_v1(
            fixture.core_program,
            fixture.release_set_id,
            &fixture.release_set,
            &inputs,
        ),
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
    let activated = activate_execution_release_set_v1(
        fixture.core_program,
        fixture.release_set_id,
        &aliased_set,
        &aliased_inputs,
    )
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
        activate_execution_release_set_v1(
            fixture.core_program,
            fixture.release_set_id,
            &aliased_set,
            &hostile_inputs,
        ),
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
        (12, Error::NonCanonicalReservedBytes),
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

    let mut zero_artifact_id = encoded;
    zero_artifact_id[ROLE_CACHE_HEADER_BYTES..ROLE_CACHE_HEADER_BYTES + 32].fill(0);
    assert_eq!(
        ActivatedExecutionReleaseSetV1::decode(&zero_artifact_id),
        Err(Error::ReleaseSet(
            dclutch_release_set_contract::Error::ZeroArtifactReleaseId
        ))
    );
}

#[test]
fn market_authority_envelope_has_one_path_to_the_activated_release_set() {
    let fixture = Fixture::distinct();
    let authority_id = content(93);
    let semantic_manifest_id = content(92);
    let manifest = ExecutionAuthorityManifestV1::new(semantic_manifest_id, fixture.release_set_id)
        .expect("distinct authority children");
    let encoded = manifest.to_bytes();
    assert_eq!(encoded.len(), EXECUTION_AUTHORITY_MANIFEST_BYTES_V1);
    assert_eq!(&encoded[..8], &EXECUTION_AUTHORITY_MANIFEST_MAGIC_V1);
    assert_eq!(&encoded[16..48], semantic_manifest_id.as_bytes());
    assert_eq!(&encoded[48..80], fixture.release_set_id.as_bytes());
    assert_eq!(ExecutionAuthorityManifestV1::decode(&encoded), Ok(manifest));

    let market = MarketIdentity::new(
        content(1),
        content(2),
        content(3),
        content(4),
        authority_id,
        7,
    );
    let authenticated =
        authenticate_market_execution_v1(market, authority_id, manifest, fixture.activate())
            .expect("immutable market join closes");
    assert_eq!(authenticated.market(), market);
    assert_eq!(
        authenticated.semantic_capability_manifest_id(),
        semantic_manifest_id
    );
    assert_eq!(
        authenticated.execution_release_set_id(),
        fixture.release_set_id
    );

    assert_eq!(
        authenticate_market_execution_v1(market, content(94), manifest, fixture.activate()),
        Err(Error::MarketAuthorityManifestMismatch)
    );
    let stale_activation = activate_execution_release_set_v1(
        fixture.core_program,
        content(95),
        &fixture.release_set,
        &fixture.inputs(),
    )
    .expect("alternate finalized identity is structurally valid at adapter boundary");
    assert_eq!(
        authenticate_market_execution_v1(market, authority_id, manifest, stale_activation),
        Err(Error::ReleaseSetSelectionMismatch)
    );
}

#[test]
fn authority_envelope_decoder_refuses_aliases_and_malformed_bytes() {
    assert_eq!(
        ExecutionAuthorityManifestV1::new(content(92), content(92)),
        Err(Error::ReleaseSetSelectionMismatch)
    );
    let encoded = ExecutionAuthorityManifestV1::new(content(92), content(91))
        .expect("valid authority envelope")
        .to_bytes();
    assert_eq!(
        ExecutionAuthorityManifestV1::decode(&encoded[..79]),
        Err(Error::InvalidLength)
    );
    let mut extended = encoded.to_vec();
    extended.push(0);
    assert_eq!(
        ExecutionAuthorityManifestV1::decode(&extended),
        Err(Error::InvalidLength)
    );
    for (offset, expected) in [
        (0, Error::InvalidMagic),
        (8, Error::UnsupportedSchema),
        (10, Error::UnsupportedArtifactProfile),
        (12, Error::NonCanonicalReservedBytes),
        (16, Error::ZeroIdentity),
        (48, Error::ZeroIdentity),
    ] {
        let mut hostile = encoded;
        if matches!(offset, 16 | 48) {
            zero_range(&mut hostile, offset, 32);
        } else {
            flip_at(&mut hostile, offset);
        }
        assert_eq!(
            ExecutionAuthorityManifestV1::decode(&hostile),
            Err(expected)
        );
    }
}
