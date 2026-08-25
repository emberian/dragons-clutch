extern crate std;

use std::{boxed::Box, vec, vec::Vec};

use dclutch_core_contract::ContentId;
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ArtifactReleaseV1, ArtifactUpgradePolicyV1,
    initialize_activation_cache_v1,
};
use dclutch_registry_svm::{AuthenticatedRoleReceiptV1, RegistryInstructionV1};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1,
    ProgramIdentityV1,
};
use solana_program::{account_info::AccountInfo, hash::hash, pubkey::Pubkey, rent::Rent};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};

use super::{
    RegistryError, RoleFrame, activate_and_write_role, authenticate_release_set_record,
    deployment_observation, process_instruction,
};

fn bytes(seed: u8) -> [u8; 32] {
    [seed; 32]
}

fn content(seed: u8) -> ContentId {
    ContentId::new(bytes(seed)).expect("nonzero content")
}

fn account(
    key: Pubkey,
    signer: bool,
    writable: bool,
    lamports: u64,
    data: Vec<u8>,
    owner: Pubkey,
    executable: bool,
) -> AccountInfo<'static> {
    AccountInfo::new(
        Box::leak(Box::new(key)),
        signer,
        writable,
        Box::leak(Box::new(lamports)),
        Box::leak(data.into_boxed_slice()),
        Box::leak(Box::new(owner)),
        executable,
    )
}

fn loader_program_bytes(programdata: Pubkey) -> Vec<u8> {
    let mut output = vec![0_u8; 36];
    output
        .get_mut(..4)
        .expect("variant bytes")
        .copy_from_slice(&2_u32.to_le_bytes());
    output
        .get_mut(4..36)
        .expect("ProgramData link bytes")
        .copy_from_slice(programdata.as_ref());
    output
}

fn immutable_programdata_bytes(slot: u64, elf: &[u8]) -> Vec<u8> {
    let mut output = vec![0_u8; 45 + elf.len()];
    output
        .get_mut(..4)
        .expect("variant bytes")
        .copy_from_slice(&3_u32.to_le_bytes());
    output
        .get_mut(4..12)
        .expect("slot bytes")
        .copy_from_slice(&slot.to_le_bytes());
    output
        .get_mut(45..)
        .expect("ELF bytes")
        .copy_from_slice(elf);
    output
}

fn finalized_record(
    registry: Pubkey,
    schema: [u8; 32],
    data: Vec<u8>,
    rent: &Rent,
) -> (AccountInfo<'static>, AccountInfo<'static>, [u8; 32]) {
    let digest = hash(&data).to_bytes();
    let raw = Pubkey::find_program_address(
        &[
            dclutch_record_contract::RAW_RECORD_PDA_SEED_V1,
            &schema,
            &digest,
        ],
        &registry,
    )
    .0;
    let staging = Pubkey::find_program_address(
        &[
            dclutch_record_contract::STAGING_CURSOR_PDA_SEED_V1,
            &schema,
            &digest,
        ],
        &registry,
    )
    .0;
    (
        account(
            raw,
            false,
            false,
            rent.minimum_balance(data.len()),
            data,
            registry,
            false,
        ),
        account(
            staging,
            false,
            false,
            0,
            Vec::new(),
            system_program::ID,
            false,
        ),
        digest,
    )
}

struct Fixture {
    registry: Pubkey,
    rent: Rent,
    release: ArtifactReleaseV1,
    artifact_id: ArtifactReleaseIdV1,
    release_set: ExecutionReleaseSetV1,
    release_set_id: ContentId,
    artifact_raw: AccountInfo<'static>,
    artifact_staging: AccountInfo<'static>,
    program: AccountInfo<'static>,
    programdata: AccountInfo<'static>,
    release_set_raw: AccountInfo<'static>,
    release_set_staging: AccountInfo<'static>,
}

impl Fixture {
    fn new() -> Self {
        let registry = Pubkey::new_from_array(bytes(7));
        let rent = Rent::default();
        let programdata_key =
            Pubkey::find_program_address(&[registry.as_ref()], &bpf_loader_upgradeable::ID).0;
        let elf = [0xa5_u8; 96];
        let slot = 77;
        let release = ArtifactReleaseV1::new(
            ProgramIdentityV1::new(registry.to_bytes()).expect("program"),
            ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader"),
            programdata_key.to_bytes(),
            content(9),
            hash(&elf).to_bytes(),
            slot,
            ArtifactUpgradePolicyV1::Immutable,
            None,
        )
        .expect("artifact release");
        let (artifact_raw, artifact_staging, artifact_digest) = finalized_record(
            registry,
            dclutch_registry_contract::ARTIFACT_RELEASE_SCHEMA_ID_V1,
            release.to_bytes().to_vec(),
            &rent,
        );
        let artifact_id = ArtifactReleaseIdV1::new(artifact_digest).expect("artifact ID");
        let binding = ExecutionRoleBindingV1::new(release.program(), artifact_id);
        let release_set = ExecutionReleaseSetV1::new(binding, binding, binding, binding, binding)
            .expect("fully aliased release set");
        let (release_set_raw, release_set_staging, release_set_digest) = finalized_record(
            registry,
            dclutch_release_set_contract::EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1,
            release_set.to_bytes().to_vec(),
            &rent,
        );
        Self {
            registry,
            rent,
            release,
            artifact_id,
            release_set,
            release_set_id: ContentId::new(release_set_digest).expect("release-set ID"),
            artifact_raw,
            artifact_staging,
            program: account(
                registry,
                false,
                false,
                1,
                loader_program_bytes(programdata_key),
                bpf_loader_upgradeable::ID,
                true,
            ),
            programdata: account(
                programdata_key,
                false,
                false,
                1,
                immutable_programdata_bytes(slot, &elf),
                bpf_loader_upgradeable::ID,
                false,
            ),
            release_set_raw,
            release_set_staging,
        }
    }

    fn role_frame(&self) -> RoleFrame<'_, 'static> {
        RoleFrame {
            artifact_record: &self.artifact_raw,
            artifact_staging: &self.artifact_staging,
            program: &self.program,
            programdata: &self.programdata,
        }
    }

    fn activated(&self) -> dclutch_registry_contract::ActivatedExecutionReleaseSetV1 {
        let frame = self.role_frame();
        let mut output = [0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
        let core_program = ProgramIdentityV1::new(self.registry.to_bytes()).expect("core program");
        initialize_activation_cache_v1(
            &mut output,
            core_program,
            self.release_set_id,
            &self.release_set,
        )
        .expect("initialize cache");
        for role in [
            ExecutionRoleV1::Core,
            ExecutionRoleV1::Claims,
            ExecutionRoleV1::Trading,
            ExecutionRoleV1::Resolution,
            ExecutionRoleV1::Custody,
        ] {
            activate_and_write_role(
                &self.registry,
                &mut output,
                core_program,
                self.release_set_id,
                &self.release_set,
                &self.rent,
                role,
                frame,
            )
            .expect("stream physical role");
        }
        ActivatedExecutionReleaseSetV1::decode(&output).expect("complete physical activation cache")
    }

    fn cache_account(&self) -> AccountInfo<'static> {
        let activated = self.activated();
        let key = Pubkey::find_program_address(
            &[
                ACTIVATION_PDA_DOMAIN_V1,
                activated.execution_release_set_id().as_bytes(),
            ],
            &self.registry,
        )
        .0;
        let data = activated.to_bytes().to_vec();
        account(
            key,
            false,
            false,
            self.rent.minimum_balance(data.len()),
            data,
            self.registry,
            false,
        )
    }
}

#[test]
fn finalized_release_set_and_five_role_activation_close_the_physical_joins() {
    let fixture = Fixture::new();
    let (release_set_id, decoded) = authenticate_release_set_record(
        &fixture.registry,
        &fixture.release_set_raw,
        &fixture.release_set_staging,
        &fixture.rent,
    )
    .expect("finalized release-set record");
    assert_eq!(release_set_id, fixture.release_set_id);
    assert_eq!(decoded, fixture.release_set);
    let activated = fixture.activated();
    assert_eq!(
        activated
            .role(ExecutionRoleV1::Custody)
            .artifact_release_id(),
        fixture.artifact_id
    );
    assert_eq!(
        activated.role(ExecutionRoleV1::Resolution).release(),
        fixture.release
    );
}

#[test]
fn current_loader_observation_binds_fixed_elf_tail_and_slot() {
    let fixture = Fixture::new();
    let observed = deployment_observation(&fixture.program, &fixture.programdata, fixture.release)
        .expect("current Loader observation");
    fixture
        .release
        .authenticate_deployment(observed)
        .expect("release accepts exact deployment");

    let stale_programdata = account(
        *fixture.programdata.key,
        false,
        false,
        1,
        immutable_programdata_bytes(fixture.release.deployment_slot() + 1, &[0xa5; 96]),
        bpf_loader_upgradeable::ID,
        false,
    );
    let stale = deployment_observation(&fixture.program, &stale_programdata, fixture.release)
        .expect("well-shaped stale observation");
    assert_eq!(
        fixture.release.authenticate_deployment(stale),
        Err(dclutch_registry_contract::Error::DeploymentSlotMismatch)
    );
}

#[test]
fn reauthentication_accepts_exact_registry_provenance_and_receipt_fields() {
    let fixture = Fixture::new();
    let cache = fixture.cache_account();
    let accounts = [cache, fixture.program.clone(), fixture.programdata.clone()];
    process_instruction(
        &fixture.registry,
        &accounts,
        &RegistryInstructionV1::Reauthenticate(ExecutionRoleV1::Resolution).to_bytes(),
    )
    .expect("reauthentication succeeds");
    // Host syscall stubs do not retain return data. Rebuild the exact receipt
    // from the same authenticated facts and exercise its hostile decoder; the
    // SBF route emits this byte sequence through `set_return_data`.
    let receipt = AuthenticatedRoleReceiptV1::new(
        ExecutionRoleV1::Resolution,
        fixture.release_set_id,
        fixture.release.program(),
        fixture.artifact_id,
        fixture.release.semantic_release_id(),
    );
    let receipt = AuthenticatedRoleReceiptV1::decode(&receipt.to_bytes()).expect("receipt");
    assert_eq!(receipt.role(), ExecutionRoleV1::Resolution);
    assert_eq!(receipt.execution_release_set_id(), fixture.release_set_id);
    assert_eq!(receipt.program(), fixture.release.program());
    assert_eq!(receipt.artifact_release_id(), fixture.artifact_id);
    assert_eq!(
        receipt.semantic_release_id(),
        fixture.release.semantic_release_id()
    );
}

#[test]
fn reauthentication_refuses_substituted_cache_and_stale_programdata() {
    let fixture = Fixture::new();
    let valid_cache = fixture.cache_account();
    let wrong_cache = account(
        Pubkey::new_from_array(bytes(99)),
        false,
        false,
        valid_cache.lamports(),
        valid_cache.try_borrow_data().expect("cache data").to_vec(),
        fixture.registry,
        false,
    );
    let accounts = [
        wrong_cache,
        fixture.program.clone(),
        fixture.programdata.clone(),
    ];
    assert_eq!(
        process_instruction(
            &fixture.registry,
            &accounts,
            &RegistryInstructionV1::Reauthenticate(ExecutionRoleV1::Core).to_bytes(),
        ),
        Err(RegistryError::ActivationCache.into())
    );

    let stale_programdata = account(
        *fixture.programdata.key,
        false,
        false,
        1,
        immutable_programdata_bytes(fixture.release.deployment_slot() + 1, &[0xa5; 96]),
        bpf_loader_upgradeable::ID,
        false,
    );
    let accounts = [
        fixture.cache_account(),
        fixture.program.clone(),
        stale_programdata,
    ];
    assert_eq!(
        process_instruction(
            &fixture.registry,
            &accounts,
            &RegistryInstructionV1::Reauthenticate(ExecutionRoleV1::Claims).to_bytes(),
        ),
        Err(RegistryError::Deployment.into())
    );
}

#[test]
fn finalized_record_refuses_substituted_owner_or_live_staging() {
    let fixture = Fixture::new();
    let data = fixture
        .release_set_raw
        .try_borrow_data()
        .expect("release-set bytes")
        .to_vec();
    let wrong_owner = account(
        *fixture.release_set_raw.key,
        false,
        false,
        fixture.release_set_raw.lamports(),
        data.clone(),
        Pubkey::new_from_array(bytes(88)),
        false,
    );
    assert_eq!(
        authenticate_release_set_record(
            &fixture.registry,
            &wrong_owner,
            &fixture.release_set_staging,
            &fixture.rent,
        ),
        Err(RegistryError::FinalizedRecord.into())
    );

    let live_staging = account(
        *fixture.release_set_staging.key,
        false,
        false,
        1,
        Vec::new(),
        system_program::ID,
        false,
    );
    assert_eq!(
        authenticate_release_set_record(
            &fixture.registry,
            &fixture.release_set_raw,
            &live_staging,
            &fixture.rent,
        ),
        Err(RegistryError::FinalizedRecord.into())
    );
}
