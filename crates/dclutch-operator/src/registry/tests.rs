use dclutch_registry_contract::{
    ACTIVATION_PDA_DOMAIN_V1, ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1, ExecutionReleaseSetV1,
    ExecutionRoleBindingV1, ExecutionRoleV1, ProgramIdentityV1,
};
use solana_program::{hash::hash, pubkey::Pubkey, rent::Rent, sysvar::SysvarSerialize};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program, sysvar};

use super::*;

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
        .expect("variant")
        .copy_from_slice(&2_u32.to_le_bytes());
    output
        .get_mut(4..)
        .expect("ProgramData")
        .copy_from_slice(programdata.as_ref());
    output
}

fn immutable_programdata_bytes(slot: u64, elf: &[u8]) -> Vec<u8> {
    let mut output = vec![0; 45 + elf.len()];
    output
        .get_mut(..4)
        .expect("variant")
        .copy_from_slice(&3_u32.to_le_bytes());
    output
        .get_mut(4..12)
        .expect("slot")
        .copy_from_slice(&slot.to_le_bytes());
    output.get_mut(45..).expect("ELF").copy_from_slice(elf);
    output
}

fn finalized_record(
    registry: Pubkey,
    schema: [u8; 32],
    data: Vec<u8>,
    rent: &Rent,
) -> (RegistryFinalizedRecordState, [u8; 32]) {
    let digest = hash(&data).to_bytes();
    let raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).0;
    let staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &registry).0;
    (
        RegistryFinalizedRecordState {
            record: observed(raw, registry, rent.minimum_balance(data.len()), false, data),
            staging_cursor: observed(staging, system_program::ID, 0, false, Vec::new()),
        },
        digest,
    )
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
    rent: Rent,
    state: RegistryActivationState,
}

impl Fixture {
    fn new() -> Self {
        let registry = Pubkey::new_from_array(bytes(7));
        let rent = Rent::default();
        let programdata =
            Pubkey::find_program_address(&[registry.as_ref()], &bpf_loader_upgradeable::ID).0;
        let elf = vec![0xa5; 96];
        let release = ArtifactReleaseV1::new(
            ProgramIdentityV1::new(registry.to_bytes()).expect("program"),
            ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader"),
            programdata.to_bytes(),
            dclutch_core_contract::ContentId::new(bytes(9)).expect("semantic release"),
            hash(&elf).to_bytes(),
            77,
            ArtifactUpgradePolicyV1::Immutable,
            None,
        )
        .expect("release");
        let (artifact_release, artifact_digest) = finalized_record(
            registry,
            ARTIFACT_RELEASE_SCHEMA_ID_V1,
            release.to_bytes().to_vec(),
            &rent,
        );
        let artifact = ArtifactReleaseIdV1::new(artifact_digest).expect("artifact");
        let binding = ExecutionRoleBindingV1::new(release.program(), artifact);
        let release_set = ExecutionReleaseSetV1::new(binding, binding, binding, binding, binding)
            .expect("aliased release set");
        let (execution_release_set, release_set_digest) = finalized_record(
            registry,
            EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1,
            release_set.to_bytes().to_vec(),
            &rent,
        );
        let release_set_id =
            dclutch_core_contract::ContentId::new(release_set_digest).expect("release set ID");
        let cache = Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_bytes()],
            &registry,
        )
        .0;
        let role = RegistryRoleState {
            artifact_release,
            program: observed(
                registry,
                bpf_loader_upgradeable::ID,
                1,
                true,
                loader_program_bytes(programdata),
            ),
            programdata: observed(
                programdata,
                bpf_loader_upgradeable::ID,
                1,
                false,
                immutable_programdata_bytes(77, &elf),
            ),
        };
        Self {
            registry,
            rent: rent.clone(),
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
                    core: role.clone(),
                    claims: role.clone(),
                    trading: role.clone(),
                    resolution: role.clone(),
                    custody: role,
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

    fn activation(&self) -> RegistryActivationReport {
        build_registry_activation_v1(self.registry, &self.state).expect("activation")
    }

    fn make_cache_existing(&mut self) {
        let report = self.activation();
        self.state.cache.owner = self.registry;
        self.state.cache.lamports = self
            .rent
            .minimum_balance(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1);
        self.state.cache.data = report.expected_cache.to_bytes().to_vec();
    }

    fn reauthentication(&self) -> RegistryReauthenticationState {
        RegistryReauthenticationState {
            registry_program: self.state.roles.core.program.clone(),
            cache: self.state.cache.clone(),
            role_program: self.state.roles.resolution.program.clone(),
            role_programdata: self.state.roles.resolution.programdata.clone(),
        }
    }
}

#[test]
fn activation_derives_exact_frame_cache_rent_and_packet_geometry() {
    let fixture = Fixture::new();
    let report = fixture.activation();
    assert_eq!(report.instruction.accounts.len(), 26);
    assert_eq!(report.cache, fixture.state.cache.key);
    assert_eq!(report.mode, RegistryActivationModeV1::Create);
    assert_eq!(
        report.cache_rent_debit_lamports,
        fixture
            .rent
            .minimum_balance(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1)
    );
    assert_eq!(report.compute.elf_bytes_hashed, 5 * 96);
    assert_eq!(report.compute.matching_measured_compute_units, None);
    assert!(
        report
            .instruction
            .accounts
            .first()
            .expect("payer")
            .is_signer
    );
    assert!(
        report
            .instruction
            .accounts
            .get(1)
            .expect("cache")
            .is_writable
    );
    let packet = compile_registry_activation_packet_v0(
        &report,
        fixture.state.payer.key,
        Hash::new_from_array(bytes(55)),
        400_000,
    )
    .expect("packet-safe activation");
    assert_eq!(packet.required_signatures, 1);
    assert_eq!(packet.wire_bytes, 509);
    assert_eq!(packet.compute_unit_limit, 400_000);
}

#[test]
fn existing_cache_requires_exact_bytes_and_reports_repeat() {
    let mut fixture = Fixture::new();
    fixture.make_cache_existing();
    let report = fixture.activation();
    assert_eq!(report.mode, RegistryActivationModeV1::Repeat);
    assert_eq!(report.cache_rent_debit_lamports, 0);

    fixture
        .state
        .cache
        .data
        .get_mut(100)
        .map(|byte| *byte ^= 1)
        .expect("cache byte");
    assert_eq!(
        build_registry_activation_v1(fixture.registry, &fixture.state),
        Err(Error::InvalidActivationCache)
    );
}

#[test]
fn activation_refuses_stale_loader_substitution_and_record_owner() {
    let mut stale = Fixture::new();
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
            .expect("slot")
            .copy_from_slice(&78_u64.to_le_bytes());
    }
    assert_eq!(
        build_registry_activation_v1(stale.registry, &stale.state),
        Err(Error::InvalidDeployment)
    );

    let mut substituted = Fixture::new();
    for role in [
        &mut substituted.state.roles.core,
        &mut substituted.state.roles.claims,
        &mut substituted.state.roles.trading,
        &mut substituted.state.roles.resolution,
        &mut substituted.state.roles.custody,
    ] {
        role.programdata
            .data
            .get_mut(45)
            .map(|byte| *byte ^= 1)
            .expect("ELF");
    }
    assert_eq!(
        build_registry_activation_v1(substituted.registry, &substituted.state),
        Err(Error::InvalidDeployment)
    );

    let mut changed_upgrade_policy = Fixture::new();
    for role in [
        &mut changed_upgrade_policy.state.roles.core,
        &mut changed_upgrade_policy.state.roles.claims,
        &mut changed_upgrade_policy.state.roles.trading,
        &mut changed_upgrade_policy.state.roles.resolution,
        &mut changed_upgrade_policy.state.roles.custody,
    ] {
        *role
            .programdata
            .data
            .get_mut(12)
            .expect("upgrade-authority tag") = 1;
        role.programdata
            .data
            .get_mut(13..45)
            .expect("upgrade authority")
            .copy_from_slice(&bytes(66));
    }
    assert_eq!(
        build_registry_activation_v1(
            changed_upgrade_policy.registry,
            &changed_upgrade_policy.state
        ),
        Err(Error::InvalidDeployment)
    );

    let mut wrong_owner = Fixture::new();
    wrong_owner.state.execution_release_set.record.owner = system_program::ID;
    assert_eq!(
        build_registry_activation_v1(wrong_owner.registry, &wrong_owner.state),
        Err(Error::InvalidFinalizedRecord)
    );
}

#[test]
fn conflicting_duplicate_observations_refuse_before_instruction_construction() {
    let mut fixture = Fixture::new();
    fixture.state.roles.claims.programdata.lamports = 2;
    assert_eq!(
        build_registry_activation_v1(fixture.registry, &fixture.state),
        Err(Error::InconsistentAlias)
    );
}

#[test]
fn reauthentication_derives_three_readonly_accounts_and_rechecks_deployment() {
    let mut fixture = Fixture::new();
    fixture.make_cache_existing();
    let state = fixture.reauthentication();
    let report = build_registry_reauthentication_v1(&state, ExecutionRoleV1::Resolution)
        .expect("reauthentication");
    assert_eq!(report.instruction.accounts.len(), 3);
    assert_eq!(report.role_program, state.role_program.key);
    assert_eq!(
        report.semantic_release_id,
        dclutch_core_contract::ContentId::new(bytes(9)).expect("semantic release")
    );
    assert!(
        report
            .instruction
            .accounts
            .iter()
            .all(|meta| !meta.is_signer && !meta.is_writable)
    );
    assert_eq!(report.compute.elf_bytes_hashed, 96);
    let packet = compile_registry_reauthentication_packet_v0(
        &report,
        Pubkey::new_from_array(bytes(91)),
        Hash::new_from_array(bytes(56)),
        80_000,
    )
    .expect("packet-safe reauthentication");
    assert_eq!(packet.required_signatures, 1);
    assert_eq!(packet.wire_bytes, 294);

    let mut stale = state;
    stale
        .role_programdata
        .data
        .get_mut(4..12)
        .expect("slot")
        .copy_from_slice(&79_u64.to_le_bytes());
    assert_eq!(
        build_registry_reauthentication_v1(&stale, ExecutionRoleV1::Resolution),
        Err(Error::InvalidDeployment)
    );
}

#[test]
fn packet_refuses_known_underbudget_and_chain_profile_overflow() {
    let mut fixture = Fixture::new();
    fixture.make_cache_existing();
    let mut report = fixture.activation();
    report.compute.matching_measured_compute_units = Some(MEASURED_REPEAT_ACTIVATION_CU_V1);
    assert_eq!(
        compile_registry_activation_packet_v0(
            &report,
            fixture.state.payer.key,
            Hash::new_from_array(bytes(57)),
            MEASURED_REPEAT_ACTIVATION_CU_V1 - 1,
        ),
        Err(Error::InvalidComputeLimit)
    );
    assert_eq!(
        compile_registry_activation_packet_v0(
            &report,
            fixture.state.payer.key,
            Hash::new_from_array(bytes(57)),
            TRANSACTION_COMPUTE_UNIT_LIMIT_V1 + 1,
        ),
        Err(Error::InvalidComputeLimit)
    );
}

#[test]
fn nonfinal_or_mismatched_snapshots_refuse() {
    let mut nonfinal = Fixture::new();
    nonfinal.state.rent_sysvar.observation.finality = Finality::Confirmed;
    assert_eq!(
        build_registry_activation_v1(nonfinal.registry, &nonfinal.state),
        Err(Error::ObservationNotFinalized)
    );

    let mut mismatch = Fixture::new();
    mismatch.state.system_program.observation.slot += 1;
    assert_eq!(
        build_registry_activation_v1(mismatch.registry, &mismatch.state),
        Err(Error::ObservationMismatch)
    );
}
