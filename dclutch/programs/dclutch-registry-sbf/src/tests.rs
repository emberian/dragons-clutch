extern crate std;

use std::{boxed::Box, vec, vec::Vec};

use dclutch_core_contract::ContentId;
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ArtifactReleaseV1, ArtifactUpgradePolicyV1,
    initialize_activation_cache_v1,
};
use dclutch_registry_svm::{
    AuthenticatedRoleReceiptV1, RegistryInstructionV1,
    batch_v2::{
        AuthenticatedRoleBatchReceiptV2, BatchErrorV2, ROLE_BATCH_RECEIPT_BYTES_V2,
        RoleBatchReceiptInputV2, RoleBatchRequestV2, RoleDeploymentObservationV2,
        encode_role_batch_receipt_v2,
    },
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1,
    ProgramIdentityV1,
};
use solana_program::{
    account_info::AccountInfo, hash::hash, pubkey::Pubkey, rent::Rent, sysvar::SysvarSerialize,
};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program, sysvar};

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
    core: Pubkey,
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
        let core = Pubkey::new_from_array(bytes(8));
        let rent = Rent::default();
        let programdata_key =
            Pubkey::find_program_address(&[core.as_ref()], &bpf_loader_upgradeable::ID).0;
        let elf = [0xa5_u8; 96];
        let slot = 77;
        let release = ArtifactReleaseV1::new(
            ProgramIdentityV1::new(core.to_bytes()).expect("program"),
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
            core,
            rent,
            release,
            artifact_id,
            release_set,
            release_set_id: ContentId::new(release_set_digest).expect("release-set ID"),
            artifact_raw,
            artifact_staging,
            program: account(
                core,
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
        initialize_activation_cache_v1(&mut output, self.release_set_id).expect("initialize cache");
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

    fn cache_account_with_writability(&self, writable: bool) -> AccountInfo<'static> {
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
            writable,
            self.rent.minimum_balance(data.len()),
            data,
            self.registry,
            false,
        )
    }

    fn cache_account(&self) -> AccountInfo<'static> {
        self.cache_account_with_writability(false)
    }

    fn runtime_plumbing(&self) -> (AccountInfo<'static>, AccountInfo<'static>) {
        let system = account(
            system_program::ID,
            false,
            false,
            1,
            Vec::new(),
            native_loader::ID,
            true,
        );
        let mut rent = account(
            sysvar::rent::ID,
            false,
            false,
            1,
            vec![0; Rent::size_of()],
            sysvar::ID,
            false,
        );
        assert_eq!(Rent::default().to_account_info(&mut rent), Some(()));
        (system, rent)
    }

    fn role_activation_accounts(&self, cache: AccountInfo<'static>) -> Vec<AccountInfo<'static>> {
        let payer = account(
            Pubkey::new_from_array(bytes(98)),
            true,
            true,
            1,
            Vec::new(),
            system_program::ID,
            false,
        );
        let (system, rent) = self.runtime_plumbing();
        let accounts = vec![
            payer,
            cache,
            self.release_set_raw.clone(),
            self.release_set_staging.clone(),
            self.artifact_raw.clone(),
            self.artifact_staging.clone(),
            self.program.clone(),
            self.programdata.clone(),
            system,
            rent,
        ];
        assert_eq!(accounts.len(), dclutch_registry_svm::REGISTRY_ACTIVATE_ROLE_ACCOUNT_COUNT_V1);
        accounts
    }

    /// A Registry-owned, correctly derived, rent-exempt cache with no role written.
    ///
    /// This is exactly what the create branch leaves behind — the account is
    /// created and its header initialized in one instruction — and it is what
    /// per-role activation walks up from.
    fn empty_cache_account(&self) -> AccountInfo<'static> {
        let key = Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, self.release_set_id.as_bytes()],
            &self.registry,
        )
        .0;
        let mut header = [0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
        initialize_activation_cache_v1(&mut header, self.release_set_id)
            .expect("initialize empty cache");
        let data = header.to_vec();
        account(
            key,
            false,
            true,
            self.rent.minimum_balance(data.len()),
            data,
            self.registry,
            false,
        )
    }
}

#[test]
fn distinct_core_and_registry_activate_all_exact_aliased_roles() {
    let fixture = Fixture::new();
    assert_ne!(fixture.registry, fixture.core);
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
    for role in [
        ExecutionRoleV1::Core,
        ExecutionRoleV1::Claims,
        ExecutionRoleV1::Trading,
        ExecutionRoleV1::Resolution,
        ExecutionRoleV1::Custody,
    ] {
        assert_eq!(activated.role(role), activated.role(ExecutionRoleV1::Core));
    }
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
fn reauthentication_accepts_distinct_core_with_exact_registry_cache_provenance() {
    let fixture = Fixture::new();
    let cache = fixture.cache_account();
    let expected_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, fixture.release_set_id.as_bytes()],
        &fixture.registry,
    )
    .0;
    assert_eq!(cache.key, &expected_cache);
    assert_eq!(cache.owner, &fixture.registry);
    assert_ne!(cache.owner, &fixture.core);
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
fn batch_reauthentication_accepts_one_cache_and_four_ordered_current_roles() {
    let fixture = Fixture::new();
    let cache = fixture.cache_account();
    let cache_digest =
        ContentId::new(hash(&cache.try_borrow_data().expect("cache bytes")).to_bytes())
            .expect("cache digest");
    let roles = [
        ExecutionRoleV1::Core,
        ExecutionRoleV1::Claims,
        ExecutionRoleV1::Trading,
        ExecutionRoleV1::Custody,
    ];
    let request = RoleBatchRequestV2::new(fixture.release_set_id, cache_digest, &roles)
        .expect("batch request");
    let mut accounts = vec![cache];
    for _ in roles {
        accounts.extend([fixture.program.clone(), fixture.programdata.clone()]);
    }
    process_instruction(&fixture.registry, &accounts, &request.to_bytes())
        .expect("one physical Registry batch");

    // Host syscall stubs do not retain return data. Reconstruct the exact
    // bytes from the authenticated facts and exercise the hostile decoder.
    let observations = roles.map(|role| {
        RoleDeploymentObservationV2::new(
            role,
            fixture.release.program(),
            fixture.release.programdata(),
            fixture.artifact_id,
            fixture.release.semantic_release_id(),
            fixture.release.deployment_slot(),
        )
        .expect("observation")
    });
    let request_bytes = request.to_bytes();
    let mut receipt_bytes = [0_u8; ROLE_BATCH_RECEIPT_BYTES_V2];
    encode_role_batch_receipt_v2(
        RoleBatchReceiptInputV2 {
            registry_program: ProgramIdentityV1::new(fixture.registry.to_bytes())
                .expect("Registry program"),
            activation_cache: *accounts.first().expect("cache").key.as_array(),
            activation_cache_digest: cache_digest,
            release_set_id: fixture.release_set_id,
            request_digest: ContentId::new(hash(&request_bytes).to_bytes())
                .expect("request digest"),
            observations: &observations,
        },
        &mut receipt_bytes,
    )
    .expect("receipt");
    let receipt = AuthenticatedRoleBatchReceiptV2::decode(&receipt_bytes).expect("batch receipt");
    assert_eq!(receipt.role_count(), 4);
    assert_eq!(receipt.role_mask(), 0b1_0111);
    for (index, role) in roles.into_iter().enumerate() {
        assert_eq!(
            receipt
                .observation(index)
                .expect("active observation")
                .expect("valid observation")
                .role(),
            role
        );
    }
}

#[test]
fn batch_reauthentication_refuses_duplicate_reorder_cache_and_deployment_substitution() {
    let fixture = Fixture::new();
    let cache = fixture.cache_account();
    let cache_digest =
        ContentId::new(hash(&cache.try_borrow_data().expect("cache bytes")).to_bytes())
            .expect("cache digest");
    assert_eq!(
        RoleBatchRequestV2::new(
            fixture.release_set_id,
            cache_digest,
            &[ExecutionRoleV1::Core, ExecutionRoleV1::Core],
        ),
        Err(BatchErrorV2::NonCanonicalRoleOrder)
    );
    assert_eq!(
        RoleBatchRequestV2::new(
            fixture.release_set_id,
            cache_digest,
            &[ExecutionRoleV1::Trading, ExecutionRoleV1::Claims],
        ),
        Err(BatchErrorV2::NonCanonicalRoleOrder)
    );

    let roles = [ExecutionRoleV1::Core, ExecutionRoleV1::Claims];
    let request = RoleBatchRequestV2::new(fixture.release_set_id, cache_digest, &roles)
        .expect("batch request");
    let wrong_digest =
        RoleBatchRequestV2::new(fixture.release_set_id, content(99), &roles).expect("bad request");
    let accounts = [
        cache.clone(),
        fixture.program.clone(),
        fixture.programdata.clone(),
        fixture.program.clone(),
        fixture.programdata.clone(),
    ];
    assert_eq!(
        process_instruction(&fixture.registry, &accounts, &wrong_digest.to_bytes()),
        Err(RegistryError::ActivationCache.into())
    );

    let wrong_cache = account(
        Pubkey::new_from_array(bytes(99)),
        false,
        false,
        cache.lamports(),
        cache.try_borrow_data().expect("cache data").to_vec(),
        fixture.registry,
        false,
    );
    let substituted_cache_accounts = [
        wrong_cache,
        fixture.program.clone(),
        fixture.programdata.clone(),
        fixture.program.clone(),
        fixture.programdata.clone(),
    ];
    assert_eq!(
        process_instruction(
            &fixture.registry,
            &substituted_cache_accounts,
            &request.to_bytes(),
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
    let stale_accounts = [
        cache,
        fixture.program.clone(),
        fixture.programdata.clone(),
        fixture.program.clone(),
        stale_programdata,
    ];
    assert_eq!(
        process_instruction(&fixture.registry, &stale_accounts, &request.to_bytes()),
        Err(RegistryError::Deployment.into())
    );
}

#[test]
fn reauthentication_refuses_substituted_core_program_and_programdata() {
    let fixture = Fixture::new();
    let substituted_core = account(
        Pubkey::new_from_array(bytes(99)),
        false,
        false,
        1,
        loader_program_bytes(*fixture.programdata.key),
        bpf_loader_upgradeable::ID,
        true,
    );
    assert_eq!(
        process_instruction(
            &fixture.registry,
            &[
                fixture.cache_account(),
                substituted_core,
                fixture.programdata.clone(),
            ],
            &RegistryInstructionV1::Reauthenticate(ExecutionRoleV1::Core).to_bytes(),
        ),
        Err(RegistryError::Deployment.into())
    );

    let substituted_programdata_key =
        Pubkey::find_program_address(&[fixture.core.as_ref(), b"substituted"], &fixture.core).0;
    let substituted_programdata = account(
        substituted_programdata_key,
        false,
        false,
        1,
        immutable_programdata_bytes(fixture.release.deployment_slot(), &[0xa5; 96]),
        bpf_loader_upgradeable::ID,
        false,
    );
    assert_eq!(
        process_instruction(
            &fixture.registry,
            &[
                fixture.cache_account(),
                fixture.program.clone(),
                substituted_programdata,
            ],
            &RegistryInstructionV1::Reauthenticate(ExecutionRoleV1::Core).to_bytes(),
        ),
        Err(RegistryError::Deployment.into())
    );
}

#[test]
fn cache_pda_and_owner_are_registry_derived_even_with_an_isolated_core() {
    let fixture = Fixture::new();
    let valid_cache = fixture.cache_account();
    let core_derived_key = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, fixture.release_set_id.as_bytes()],
        &fixture.core,
    )
    .0;
    let core_owned_cache = account(
        core_derived_key,
        false,
        false,
        valid_cache.lamports(),
        valid_cache.try_borrow_data().expect("cache data").to_vec(),
        fixture.core,
        false,
    );
    assert_eq!(
        process_instruction(
            &fixture.registry,
            &[
                core_owned_cache,
                fixture.program.clone(),
                fixture.programdata.clone(),
            ],
            &RegistryInstructionV1::Reauthenticate(ExecutionRoleV1::Core).to_bytes(),
        ),
        Err(RegistryError::ActivationCache.into())
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

#[test]
fn per_role_activation_walks_up_and_is_byte_identical_when_repeated() {
    let fixture = Fixture::new();
    let cache = fixture.empty_cache_account();
    let accounts = fixture.role_activation_accounts(cache);
    let roles = [
        ExecutionRoleV1::Core,
        ExecutionRoleV1::Claims,
        ExecutionRoleV1::Trading,
        ExecutionRoleV1::Resolution,
        ExecutionRoleV1::Custody,
    ];

    for (written, role) in roles.into_iter().enumerate() {
        process_instruction(
            &fixture.registry,
            &accounts,
            &RegistryInstructionV1::ActivateRole(role).to_bytes(),
        )
        .expect("one role activates into the shared cache");
        let data = accounts
            .get(1)
            .expect("activation cache account")
            .try_borrow_data()
            .expect("cache after one role");
        // Nothing may read a half-activated release set: the complete view is
        // exactly what a consumer decodes, and it must refuse until the last
        // role lands.
        let complete = ActivatedExecutionReleaseSetV1::decode(&data).is_ok();
        assert_eq!(
            complete,
            written + 1 == roles.len(),
            "cache with {} of {} roles written",
            written + 1,
            roles.len()
        );
    }

    let after_walk_up = accounts
        .get(1)
        .expect("activation cache account")
        .try_borrow_data()
        .expect("cache after walk-up")
        .to_vec();
    assert_eq!(
        ActivatedExecutionReleaseSetV1::decode(&after_walk_up),
        Ok(fixture.activated())
    );

    for role in roles {
        process_instruction(
            &fixture.registry,
            &accounts,
            &RegistryInstructionV1::ActivateRole(role).to_bytes(),
        )
        .expect("repeated activation of an already-written role is idempotent");
        assert_eq!(
            accounts
                .get(1)
                .expect("activation cache account")
                .try_borrow_data()
                .expect("cache after repeat activation")
                .as_ref(),
            after_walk_up.as_slice()
        );
    }
}

#[test]
fn retired_five_role_activation_frame_refuses() {
    let fixture = Fixture::new();
    // The retired five-role `Activate` wire was action 0 with role 0, which now
    // names `ActivateRole(Core)` — strictly less authority, never more. Its
    // 26-account frame cannot reach the ten-account route.
    let mut retired_frame = fixture.role_activation_accounts(fixture.empty_cache_account());
    for _ in 0..4 {
        retired_frame.insert(
            8,
            fixture.artifact_raw.clone(),
        );
        retired_frame.insert(9, fixture.artifact_staging.clone());
        retired_frame.insert(10, fixture.program.clone());
        retired_frame.insert(11, fixture.programdata.clone());
    }
    assert_eq!(retired_frame.len(), 26);
    assert_eq!(
        process_instruction(
            &fixture.registry,
            &retired_frame,
            &RegistryInstructionV1::ActivateRole(ExecutionRoleV1::Core).to_bytes(),
        ),
        Err(RegistryError::AccountFrame.into())
    );
    let cache = retired_frame
        .get(1)
        .expect("activation cache account")
        .try_borrow_data()
        .expect("cache after refusal");
    assert_eq!(
        cache.as_ref(),
        fixture
            .empty_cache_account()
            .try_borrow_data()
            .expect("pristine cache")
            .as_ref(),
        "a refused stale-frame activation admits no role"
    );
}

#[test]
fn role_activation_refuses_a_hostile_account_frame() {
    let fixture = Fixture::new();
    let canonical = fixture.role_activation_accounts(fixture.empty_cache_account());
    let data = RegistryInstructionV1::ActivateRole(ExecutionRoleV1::Core).to_bytes();

    let mut short = canonical.clone();
    short.pop();
    assert_eq!(
        process_instruction(&fixture.registry, &short, &data),
        Err(RegistryError::AccountFrame.into())
    );

    let mut long = canonical.clone();
    long.push(fixture.programdata.clone());
    assert_eq!(
        process_instruction(&fixture.registry, &long, &data),
        Err(RegistryError::AccountFrame.into())
    );

    let mut readonly_cache = canonical.clone();
    let cache = fixture.empty_cache_account();
    readonly_cache[1] = account(
        *cache.key,
        false,
        false,
        cache.lamports(),
        cache.try_borrow_data().expect("cache bytes").to_vec(),
        fixture.registry,
        false,
    );
    assert_eq!(
        process_instruction(&fixture.registry, &readonly_cache, &data),
        Err(RegistryError::AccountFrame.into())
    );

    let mut signer_programdata = canonical.clone();
    signer_programdata[7] = account(
        *fixture.programdata.key,
        true,
        false,
        fixture.programdata.lamports(),
        fixture
            .programdata
            .try_borrow_data()
            .expect("programdata bytes")
            .to_vec(),
        bpf_loader_upgradeable::ID,
        false,
    );
    assert_eq!(
        process_instruction(&fixture.registry, &signer_programdata, &data),
        Err(RegistryError::AccountFrame.into())
    );
}

#[test]
fn role_activation_refuses_a_substituted_deployment() {
    let fixture = Fixture::new();
    let mut accounts = fixture.role_activation_accounts(fixture.empty_cache_account());
    // Same well-shaped Loader state, different deployed bytes: first admission
    // is the sole site that checks the artifact record's claimed ELF digest
    // against what is actually deployed, and it must still hash to do it.
    accounts[7] = account(
        *fixture.programdata.key,
        false,
        false,
        fixture.programdata.lamports(),
        immutable_programdata_bytes(fixture.release.deployment_slot(), &[0x5a; 96]),
        bpf_loader_upgradeable::ID,
        false,
    );
    assert_eq!(
        process_instruction(
            &fixture.registry,
            &accounts,
            &RegistryInstructionV1::ActivateRole(ExecutionRoleV1::Core).to_bytes(),
        ),
        Err(RegistryError::Release.into())
    );
    let cache = accounts
        .get(1)
        .expect("activation cache account")
        .try_borrow_data()
        .expect("cache after refusal");
    assert_eq!(
        cache.as_ref(),
        fixture
            .empty_cache_account()
            .try_borrow_data()
            .expect("pristine cache")
            .as_ref(),
        "a refused role activation leaves no admitted role behind"
    );
}
