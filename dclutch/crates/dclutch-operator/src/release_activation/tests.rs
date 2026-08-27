use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{ACTIVATION_PDA_DOMAIN_V1, ARTIFACT_RELEASE_SCHEMA_ID_V1};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1, ExecutionReleaseSetV1,
    ExecutionRoleBindingV1,
};
use dclutch_release_tool::{
    BuildMetadataV1, CheckedReleaseV1, ReleaseEvidenceV1, artifact_release_from_checked,
    build_checked_execution_release_set, build_checked_release,
};
use solana_program::{
    account_info::AccountInfo, hash::hash, pubkey::Pubkey, rent::Rent, sysvar::SysvarSerialize,
};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program, sysvar};

use super::*;
use crate::registry::{
    RegistryActivationState, RegistryFinalizedRecordState, RegistryRoleSetState, RegistryRoleState,
    TRANSACTION_COMPUTE_UNIT_LIMIT_V1,
};
use crate::{Finality, Observation, ObservedAccount};

fn bytes(seed: u8) -> [u8; 32] {
    [seed; 32]
}

fn observation() -> Observation {
    Observation {
        slot: 88,
        unix_timestamp: 1_800_000_000,
        finality: Finality::Finalized,
    }
}

fn observed(
    key: Pubkey,
    owner: Pubkey,
    lamports: u64,
    executable: bool,
    data: Vec<u8>,
) -> ObservedAccount {
    ObservedAccount {
        observation: observation(),
        key,
        owner,
        lamports,
        executable,
        data,
    }
}

fn loader_program_bytes(programdata: Pubkey) -> Vec<u8> {
    let mut output = vec![0; 36];
    output
        .get_mut(..4)
        .expect("Loader variant")
        .copy_from_slice(&2_u32.to_le_bytes());
    output
        .get_mut(4..)
        .expect("ProgramData identity")
        .copy_from_slice(programdata.as_ref());
    output
}

fn immutable_programdata_bytes(slot: u64, elf: &[u8]) -> Vec<u8> {
    let mut output = vec![0; 45 + elf.len()];
    output
        .get_mut(..4)
        .expect("Loader variant")
        .copy_from_slice(&3_u32.to_le_bytes());
    output
        .get_mut(4..12)
        .expect("deployment slot")
        .copy_from_slice(&slot.to_le_bytes());
    output.get_mut(45..).expect("ELF tail").copy_from_slice(elf);
    output
}

fn sbf_elf(seed: u8) -> Vec<u8> {
    let mut elf = vec![seed; 64];
    elf.get_mut(..4)
        .expect("ELF magic")
        .copy_from_slice(&[0x7f, b'E', b'L', b'F']);
    *elf.get_mut(4).expect("ELF class") = 2;
    *elf.get_mut(5).expect("ELF byte order") = 1;
    *elf.get_mut(6).expect("ELF version") = 1;
    elf.get_mut(16..18)
        .expect("ELF type")
        .copy_from_slice(&3_u16.to_le_bytes());
    elf.get_mut(18..20)
        .expect("ELF machine")
        .copy_from_slice(&263_u16.to_le_bytes());
    elf.get_mut(20..24)
        .expect("ELF object version")
        .copy_from_slice(&1_u32.to_le_bytes());
    elf.get_mut(52..54)
        .expect("ELF header width")
        .copy_from_slice(&64_u16.to_le_bytes());
    elf
}

fn checked_release(
    registry: Pubkey,
    programdata: Pubkey,
    elf: &[u8],
    semantic_seed: u8,
    source_seed: u8,
) -> CheckedReleaseV1 {
    let program_data = loader_program_bytes(programdata);
    let programdata_data = immutable_programdata_bytes(77, elf);
    let metadata = BuildMetadataV1::parse(&format!(
        concat!(
            "dclutch-release-metadata-v1\n",
            "semantic_kind=capability\n",
            "program_id={}\n",
            "programdata_id={}\n",
            "loader_program_id={}\n",
            "program_owner={}\n",
            "program_executable=true\n",
            "programdata_owner={}\n",
            "programdata_executable=false\n",
            "source_digest={}\n",
            "cargo_lock_digest={}\n",
            "source_revision=fixture-revision\n",
            "rustc_version=rustc-fixture\n",
            "solana_version=solana-fixture\n",
            "cargo_build_sbf_version=cargo-build-sbf-fixture\n",
            "target_triple=sbf-solana-solana\n",
            "build_command=cargo-build-sbf --fixture\n",
            "assumption=hostile fixture only\n",
        ),
        hex(registry.as_ref()),
        hex(programdata.as_ref()),
        hex(bpf_loader_upgradeable::ID.as_ref()),
        hex(bpf_loader_upgradeable::ID.as_ref()),
        hex(bpf_loader_upgradeable::ID.as_ref()),
        hex(&bytes(source_seed)),
        hex(&bytes(99)),
    ))
    .expect("canonical metadata");
    let semantic_preimage = [semantic_seed; 16];
    build_checked_release(ReleaseEvidenceV1 {
        elf,
        semantic_preimage: &semantic_preimage,
        program_account_data: &program_data,
        programdata_account_data: &programdata_data,
        metadata: &metadata,
    })
    .expect("checked release")
}

fn finalized_record(
    registry: Pubkey,
    schema: [u8; 32],
    data: Vec<u8>,
    rent: &Rent,
) -> RegistryFinalizedRecordState {
    let digest = hash(&data).to_bytes();
    let raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).0;
    let staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &registry).0;
    RegistryFinalizedRecordState {
        record: observed(raw, registry, rent.minimum_balance(data.len()), false, data),
        staging_cursor: observed(staging, system_program::ID, 0, false, Vec::new()),
    }
}

fn rent_account(rent: Rent) -> ObservedAccount {
    let mut lamports = 1;
    let mut data = vec![0; Rent::size_of()];
    let key = sysvar::rent::ID;
    let owner = sysvar::ID;
    let mut info = AccountInfo::new(&key, false, false, &mut lamports, &mut data, &owner, false);
    assert_eq!(rent.to_account_info(&mut info), Some(()));
    observed(key, owner, 1, false, data)
}

struct Fixture {
    registry: Pubkey,
    checked_releases: [CheckedReleaseV1; EXECUTION_ROLE_COUNT_V1],
    checked_release_set: CheckedExecutionReleaseSetV1,
    state: RegistryActivationState,
}

impl Fixture {
    fn new(semantic_seed: u8, source_seed: u8) -> Self {
        let registry = Pubkey::new_from_array(bytes(7));
        let programs = [
            registry,
            Pubkey::new_from_array(bytes(8)),
            Pubkey::new_from_array(bytes(9)),
            Pubkey::new_from_array(bytes(10)),
            Pubkey::new_from_array(bytes(11)),
        ];
        let programdatas = programs.map(|program| {
            Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
        });
        let elfs: [Vec<u8>; EXECUTION_ROLE_COUNT_V1] = std::array::from_fn(|index| {
            sbf_elf(0xa5_u8.saturating_add(u8::try_from(index).expect("role index")))
        });
        let checked_releases: [CheckedReleaseV1; EXECUTION_ROLE_COUNT_V1] =
            std::array::from_fn(|index| {
                checked_release(
                    programs.get(index).copied().expect("role program"),
                    programdatas.get(index).copied().expect("role ProgramData"),
                    elfs.get(index).expect("role ELF"),
                    semantic_seed.saturating_add(u8::try_from(index).expect("role index")),
                    source_seed.saturating_add(u8::try_from(index).expect("role index")),
                )
            });
        let artifacts = std::array::from_fn(|index| {
            artifact_release_from_checked(
                checked_releases.get(index).expect("role checked release"),
            )
            .expect("artifact projection")
        });
        let bindings = artifacts.map(|artifact| {
            let artifact_id = ArtifactReleaseIdV1::new(hash(&artifact.to_bytes()).to_bytes())
                .expect("artifact ID");
            ExecutionRoleBindingV1::new(artifact.program(), artifact_id)
        });
        let [
            core_binding,
            claims_binding,
            trading_binding,
            resolution_binding,
            custody_binding,
        ] = bindings;
        let release_set = ExecutionReleaseSetV1::new(
            core_binding,
            claims_binding,
            trading_binding,
            resolution_binding,
            custody_binding,
        )
        .expect("five-role release set");
        let checked_release_set =
            build_checked_execution_release_set(release_set, checked_releases.each_ref())
                .expect("checked multiprogram set");
        let rent = Rent::default();
        let artifact_releases = artifacts.map(|artifact| {
            finalized_record(
                registry,
                ARTIFACT_RELEASE_SCHEMA_ID_V1,
                artifact.to_bytes().to_vec(),
                &rent,
            )
        });
        let execution_release_set = finalized_record(
            registry,
            EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1,
            release_set.to_bytes().to_vec(),
            &rent,
        );
        let release_set_id = checked_release_set
            .execution_release_set_id()
            .expect("release-set ID");
        let cache = Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_bytes()],
            &registry,
        )
        .0;
        let roles = std::array::from_fn(|index| RegistryRoleState {
            artifact_release: artifact_releases
                .get(index)
                .cloned()
                .expect("role artifact release"),
            program: observed(
                programs.get(index).copied().expect("role program"),
                bpf_loader_upgradeable::ID,
                1,
                true,
                loader_program_bytes(programdatas.get(index).copied().expect("role ProgramData")),
            ),
            programdata: observed(
                programdatas.get(index).copied().expect("role ProgramData"),
                bpf_loader_upgradeable::ID,
                1,
                false,
                immutable_programdata_bytes(77, elfs.get(index).expect("role ELF")),
            ),
        });
        let [core, claims, trading, resolution, custody] = roles;
        Self {
            registry,
            checked_releases,
            checked_release_set,
            state: RegistryActivationState {
                payer: observed(
                    Pubkey::new_from_array(bytes(90)),
                    system_program::ID,
                    100_000_000,
                    false,
                    Vec::new(),
                ),
                cache: observed(cache, system_program::ID, 0, false, Vec::new()),
                execution_release_set,
                roles: RegistryRoleSetState {
                    core,
                    claims,
                    trading,
                    resolution,
                    custody,
                },
                system_program: observed(
                    system_program::ID,
                    native_loader::ID,
                    1,
                    true,
                    Vec::new(),
                ),
                rent_sysvar: rent_account(rent),
            },
        }
    }

    fn build(&self) -> Result<CheckedRegistryActivationPlanV1, Error> {
        build_checked_registry_activation_packet_v1(
            self.registry,
            self.checked_release_set,
            self.checked_releases.each_ref(),
            &self.state,
            self.state.payer.key,
            Hash::new_from_array(bytes(55)),
            400_000,
        )
    }
}

#[test]
fn exact_checked_evidence_builds_existing_activation_and_deterministic_projection() {
    let fixture = Fixture::new(41, 71);
    let plan = fixture.build().expect("checked activation");
    assert_eq!(plan.activation.roles.len(), 5);
    assert_eq!(plan.packets.len(), 5);
    for (role_plan, packet) in plan.activation.roles.iter().zip(plan.packets.iter()) {
        assert_eq!(
            role_plan.instruction.accounts.len(),
            dclutch_registry_svm::REGISTRY_ACTIVATE_ROLE_ACCOUNT_COUNT_V1
        );
        assert_eq!(packet.required_signatures, 1);
        assert!(packet.wire_bytes <= crate::versioned::PACKET_DATA_BYTES);
    }
    assert_eq!(
        plan.activation.expected_cache.release_set_projection(),
        Ok(fixture.checked_release_set.release_set())
    );

    let evidence = plan.render_evidence_text().expect("evidence projection");
    assert_eq!(
        evidence,
        plan.render_evidence_text().expect("stable projection")
    );
    let keys = evidence
        .lines()
        .map(|line| line.split_once('=').expect("key/value").0)
        .collect::<Vec<_>>();
    let head = keys
        .iter()
        .position(|key| *key == "activation_projection")
        .expect("activation projection header");
    assert_eq!(
        keys.get(head..head + 10),
        Some(
            [
                "activation_projection",
                "observation_slot",
                "observation_unix_timestamp",
                "observation_finality",
                "registry_program_id",
                "activation_cache",
                "activation_mode",
                "cache_rent_debit_lamports",
                "elf_bytes_hashed_total",
                "activation_transactions",
            ]
            .as_slice()
        )
    );
    assert!(keys.contains(&"matching_measured_compute_units"));
    // One packet projection per activation transaction, in canonical role order.
    for role in ["core", "claims", "trading", "resolution", "custody"] {
        for key in [
            "role_elf_bytes_hashed",
            "unsigned_message_sha256",
            "packet_wire_bytes",
            "required_signatures",
            "compute_unit_limit",
            "measured_headroom",
        ] {
            assert!(
                keys.contains(&format!("{key}_{role}").as_str()),
                "missing {key}_{role}"
            );
        }
    }
    assert!(evidence.ends_with("measured_headroom_custody=none\n"));
}

#[test]
fn invented_checked_release_identity_refuses_even_when_artifact_is_unchanged() {
    let fixture = Fixture::new(41, 71);
    let alternate_evidence = Fixture::new(41, 72);
    for (expected, alternate) in fixture
        .checked_releases
        .iter()
        .zip(alternate_evidence.checked_releases.iter())
    {
        assert_eq!(
            artifact_release_from_checked(expected),
            artifact_release_from_checked(alternate)
        );
    }
    assert_eq!(
        build_checked_registry_activation_packet_v1(
            fixture.registry,
            fixture.checked_release_set,
            alternate_evidence.checked_releases.each_ref(),
            &fixture.state,
            fixture.state.payer.key,
            Hash::new_from_array(bytes(55)),
            400_000,
        ),
        Err(Error::IdentityMismatch)
    );

    let mut reordered_roles = fixture.checked_releases.each_ref();
    reordered_roles.swap(1, 2);
    assert_eq!(
        build_checked_registry_activation_packet_v1(
            fixture.registry,
            fixture.checked_release_set,
            reordered_roles,
            &fixture.state,
            fixture.state.payer.key,
            Hash::new_from_array(bytes(55)),
            400_000,
        ),
        Err(Error::ReleaseTool(
            dclutch_release_tool::Error::InvalidExecutionReleaseSet
        ))
    );
}

#[test]
fn self_consistent_onchain_release_substitution_refuses_checked_manifest_join() {
    let checked = Fixture::new(41, 71);
    let substituted_chain = Fixture::new(42, 71);
    assert_eq!(
        build_checked_registry_activation_packet_v1(
            substituted_chain.registry,
            checked.checked_release_set,
            checked.checked_releases.each_ref(),
            &substituted_chain.state,
            substituted_chain.state.payer.key,
            Hash::new_from_array(bytes(55)),
            400_000,
        ),
        Err(Error::IdentityMismatch)
    );
}

#[test]
fn stale_loader_and_oversized_packet_budget_preserve_registry_refusals() {
    let mut stale = Fixture::new(41, 71);
    for role in [
        &mut stale.state.roles.core,
        &mut stale.state.roles.claims,
        &mut stale.state.roles.trading,
        &mut stale.state.roles.resolution,
        &mut stale.state.roles.custody,
    ] {
        role.programdata
            .data
            .get_mut(4..12)
            .expect("deployment slot")
            .copy_from_slice(&78_u64.to_le_bytes());
    }
    assert_eq!(
        stale.build(),
        Err(Error::Registry(RegistryError::InvalidDeployment))
    );

    let fixture = Fixture::new(41, 71);
    assert_eq!(
        build_checked_registry_activation_packet_v1(
            fixture.registry,
            fixture.checked_release_set,
            fixture.checked_releases.each_ref(),
            &fixture.state,
            fixture.state.payer.key,
            Hash::new_from_array(bytes(55)),
            TRANSACTION_COMPUTE_UNIT_LIMIT_V1 + 1,
        ),
        Err(Error::Registry(RegistryError::InvalidComputeLimit))
    );
}
