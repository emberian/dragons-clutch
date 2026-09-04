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
    batch_v2::{BatchErrorV2, ROLE_BATCH_REQUEST_MAGIC_V2, RoleBatchRequestV2},
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

/// The System Program account as a runtime presents it, and nowhere else.
///
/// Every fixture that stands this account up goes through here, because a
/// fixture that gets it wrong is invisible: the whole point of the account is
/// that a caller cannot substitute anything for it, so a test can only ever
/// compare the program against whatever fiction the fixture supplies.
///
/// The two coordinates that are easy to get wrong and were: it is EXECUTABLE --
/// a native program account is a program -- and it carries its own name as
/// data rather than being empty. A consent slot built without the executable
/// bit let the lineage route's conjuncts 1 and 6 both pass here while being
/// mutually unsatisfiable on any real runtime, and a `DATALESS` clause once
/// admitted only synthesized accounts and refused every real one.
fn system_program_account() -> AccountInfo<'static> {
    account(
        system_program::ID,
        false,
        false,
        1,
        Vec::from(&b"system_program"[..]),
        native_loader::ID,
        true,
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
        let system = system_program_account();
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
        assert_eq!(
            accounts.len(),
            dclutch_registry_svm::REGISTRY_ACTIVATE_ROLE_ACCOUNT_COUNT_V1
        );
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

// The standalone DCLTRGB2 route is deleted; `batch_v2::authenticate_request` is
// not. It is what `continuation_v1` and `hot_continuation_v2` reach in-process,
// so these two tests drive it directly instead of through a dispatch arm.
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
    assert_eq!(request.role_mask(), 0b1_0111);
    let mut accounts = vec![cache];
    for _ in roles {
        accounts.extend([fixture.program.clone(), fixture.programdata.clone()]);
    }
    let authenticated =
        crate::batch_v2::authenticate_request(&fixture.registry, &accounts, request)
            .expect("one read-only Registry batch");
    assert_eq!(authenticated.cache_digest, cache_digest);
    assert_eq!(authenticated.observations.len(), roles.len());
    for (observation, role) in authenticated.observations.iter().zip(roles) {
        assert_eq!(observation.role(), role);
        assert_eq!(observation.program(), fixture.release.program());
        assert_eq!(observation.programdata(), fixture.release.programdata());
        assert_eq!(observation.artifact_release_id(), fixture.artifact_id);
        assert_eq!(
            observation.semantic_release_id(),
            fixture.release.semantic_release_id()
        );
        assert_eq!(
            observation.deployment_slot(),
            fixture.release.deployment_slot()
        );
    }

    // And the retired entry route is gone from dispatch: a canonical DCLTRGB2
    // request no longer selects a handler, it refuses as an unknown instruction.
    assert!(request.to_bytes().starts_with(&ROLE_BATCH_REQUEST_MAGIC_V2));
    assert_eq!(
        process_instruction(&fixture.registry, &accounts, &request.to_bytes()),
        Err(RegistryError::Instruction.into())
    );
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
        crate::batch_v2::authenticate_request(&fixture.registry, &accounts, wrong_digest).err(),
        Some(RegistryError::ActivationCache.into())
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
        crate::batch_v2::authenticate_request(
            &fixture.registry,
            &substituted_cache_accounts,
            request,
        )
        .err(),
        Some(RegistryError::ActivationCache.into())
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
        crate::batch_v2::authenticate_request(&fixture.registry, &stale_accounts, request).err(),
        Some(RegistryError::Deployment.into())
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
        authenticate_release_set_record(&fixture.registry, &fixture.release_set_raw, &live_staging,),
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
        retired_frame.insert(8, fixture.artifact_raw.clone());
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
    *readonly_cache.get_mut(1).expect("activation cache account") = account(
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
    *signer_programdata.get_mut(7).expect("programdata account") = account(
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
    *accounts.get_mut(7).expect("programdata account") = account(
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

// ---------------------------------------------------------------------------
// Release-set lineage: `DeclareSuccessor`
// ---------------------------------------------------------------------------

use dclutch_registry_activation_auth_v1::{
    release_lineage_address_and_bump_v1, release_lineage_address_v1,
};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_MAGIC_V1, ACTIVATED_EXECUTION_RELEASE_SET_PROFILE_V1,
    ACTIVATED_EXECUTION_RELEASE_SET_SCHEMA_VERSION_V1, ACTIVATED_ROLE_BYTES_V1,
    RELEASE_LINEAGE_BYTES_V1, ReleaseLineageV1,
};
use dclutch_release_set_contract::{EXECUTION_ROLE_COUNT_V1, EXECUTION_ROLE_ORDER_V1};
use solana_program::{entrypoint::ProgramResult, program_error::ProgramError};

fn copied<T: Copy, const N: usize>(values: &[T; N], index: usize) -> T {
    values
        .get(index)
        .copied()
        .expect("fixture index is in range")
}
use dclutch_registry_svm::lineage_v1::{
    DECLARE_SUCCESSOR_ACCOUNT_COUNT_V1, DECLARE_SUCCESSOR_LINEAGE_ACCOUNT_V1,
    DECLARE_SUCCESSOR_MAGIC_V1, DECLARE_SUCCESSOR_PREDECESSOR_CACHE_ACCOUNT_V1,
    DECLARE_SUCCESSOR_SUCCESSOR_CACHE_ACCOUNT_V1, DeclareSuccessorV1,
};

const LINEAGE_CACHE_ROLES_OFFSET: usize = 48;

/// One role's contribution to a hand-built activation cache.
#[derive(Clone, Copy)]
struct LineageRole {
    artifact_id: ArtifactReleaseIdV1,
    release: ArtifactReleaseV1,
}

/// Build the exact bytes the Registry would have written for one cache.
///
/// The declaration route reads only these two accounts and never observes a
/// deployment, so composing the cache directly is the same input the route sees
/// on chain — it is not a shortcut past a check the route makes.
fn lineage_cache_bytes(
    release_set_id: ContentId,
    roles: [LineageRole; EXECUTION_ROLE_COUNT_V1],
) -> Vec<u8> {
    let mut output = vec![0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    output
        .get_mut(..8)
        .expect("magic in bounds")
        .copy_from_slice(&ACTIVATED_EXECUTION_RELEASE_SET_MAGIC_V1);
    output
        .get_mut(8..10)
        .expect("schema in bounds")
        .copy_from_slice(&ACTIVATED_EXECUTION_RELEASE_SET_SCHEMA_VERSION_V1.to_le_bytes());
    output
        .get_mut(10..12)
        .expect("profile in bounds")
        .copy_from_slice(&ACTIVATED_EXECUTION_RELEASE_SET_PROFILE_V1.to_le_bytes());
    output
        .get_mut(16..48)
        .expect("release set id in bounds")
        .copy_from_slice(release_set_id.as_bytes());
    for role in EXECUTION_ROLE_ORDER_V1 {
        let index = role.role_index();
        let entry = copied(&roles, index);
        let offset = LINEAGE_CACHE_ROLES_OFFSET + index * ACTIVATED_ROLE_BYTES_V1;
        output
            .get_mut(offset..offset + 32)
            .expect("artifact id in bounds")
            .copy_from_slice(entry.artifact_id.as_bytes());
        output
            .get_mut(offset + 32..offset + ACTIVATED_ROLE_BYTES_V1)
            .expect("release in bounds")
            .copy_from_slice(&entry.release.to_bytes());
    }
    output
}

/// Two release sets that differ in exactly the roles named by `moved`.
struct LineageFixture {
    registry: Pubkey,
    rent: Rent,
    predecessor_id: ContentId,
    successor_id: ContentId,
    authority: [Pubkey; EXECUTION_ROLE_COUNT_V1],
    predecessor: [LineageRole; EXECUTION_ROLE_COUNT_V1],
    successor: [LineageRole; EXECUTION_ROLE_COUNT_V1],
}

fn lineage_role(
    program: Pubkey,
    artifact_seed: u8,
    slot: u64,
    authority: Option<[u8; 32]>,
) -> LineageRole {
    let programdata =
        Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0;
    let policy = if authority.is_some() {
        ArtifactUpgradePolicyV1::ExactAuthority
    } else {
        ArtifactUpgradePolicyV1::Immutable
    };
    LineageRole {
        artifact_id: ArtifactReleaseIdV1::new(bytes(artifact_seed)).expect("artifact id"),
        release: ArtifactReleaseV1::new(
            ProgramIdentityV1::new(program.to_bytes()).expect("program"),
            ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader"),
            programdata.to_bytes(),
            content(artifact_seed.wrapping_add(0x40) | 1),
            bytes(artifact_seed.wrapping_add(0x60) | 1),
            slot,
            policy,
            authority,
        )
        .expect("artifact release"),
    }
}

impl LineageFixture {
    /// `moved[i]` is whether role `i`'s artifact changed across the hop.
    fn with_moved(moved: [bool; EXECUTION_ROLE_COUNT_V1]) -> Self {
        let registry = Pubkey::new_from_array(bytes(0x07));
        let mut authority = [Pubkey::default(); EXECUTION_ROLE_COUNT_V1];
        let mut predecessor = [lineage_role(
            Pubkey::new_from_array(bytes(0x30)),
            0x30,
            100,
            Some(bytes(0x50)),
        ); EXECUTION_ROLE_COUNT_V1];
        let mut successor = predecessor;
        for role in EXECUTION_ROLE_ORDER_V1 {
            let index = role.role_index();
            let seed = 0x30 + u8::try_from(index).expect("role index is small");
            let key = Pubkey::new_from_array(bytes(0x50 + u8::try_from(index).expect("small")));
            let program = Pubkey::new_from_array(bytes(seed));
            let before = lineage_role(program, seed, 100, Some(key.to_bytes()));
            // A moved role gets a new artifact id AND a strictly later slot,
            // which is what the Loader guarantees an upgrade produces.
            let after = if copied(&moved, index) {
                lineage_role(program, seed | 0x80, 200, Some(key.to_bytes()))
            } else {
                before
            };
            if let Some(slot) = authority.get_mut(index) {
                *slot = key;
            }
            if let Some(slot) = predecessor.get_mut(index) {
                *slot = before;
            }
            if let Some(slot) = successor.get_mut(index) {
                *slot = after;
            }
        }
        Self {
            registry,
            rent: Rent::default(),
            predecessor_id: content(0x11),
            successor_id: content(0x22),
            authority,
            predecessor,
            successor,
        }
    }

    fn new() -> Self {
        // Core and Trading moved; Claims, Resolution and Custody did not.
        Self::with_moved([true, false, true, false, false])
    }

    fn cache_account(
        &self,
        id: ContentId,
        roles: [LineageRole; EXECUTION_ROLE_COUNT_V1],
    ) -> AccountInfo<'static> {
        let key = Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, id.as_bytes()],
            &self.registry,
        )
        .0;
        let data = lineage_cache_bytes(id, roles);
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

    fn pristine_lineage_account(&self) -> AccountInfo<'static> {
        account(
            release_lineage_address_v1(&self.registry, &self.predecessor_id.to_bytes()),
            false,
            true,
            0,
            Vec::new(),
            system_program::ID,
            false,
        )
    }

    fn authority_slot(&self, role: ExecutionRoleV1, moved: bool) -> AccountInfo<'static> {
        if moved {
            account(
                copied(&self.authority, role.role_index()),
                true,
                false,
                1,
                Vec::new(),
                system_program::ID,
                false,
            )
        } else {
            // Conjunct 6 requires exactly this account here, and it is the SAME
            // account the frame's System slot holds -- so it is built the same
            // way. Building it without the executable bit is what let this suite
            // pass every unmoved-role assertion against a frame no runtime can
            // present, and hid a route that refused `AccountFrame` on chain for
            // every hop with an unmoved role.
            system_program_account()
        }
    }

    /// The canonical eleven-account declaration frame.
    fn accounts(&self, moved: [bool; EXECUTION_ROLE_COUNT_V1]) -> Vec<AccountInfo<'static>> {
        let payer = account(
            Pubkey::new_from_array(bytes(0x98)),
            true,
            true,
            1_000_000_000,
            Vec::new(),
            system_program::ID,
            false,
        );
        let (system, rent) = self.runtime_plumbing();
        let mut accounts = vec![
            payer,
            self.pristine_lineage_account(),
            self.cache_account(self.predecessor_id, self.predecessor),
            self.cache_account(self.successor_id, self.successor),
        ];
        for role in EXECUTION_ROLE_ORDER_V1 {
            accounts.push(self.authority_slot(role, copied(&moved, role.role_index())));
        }
        accounts.push(system);
        accounts.push(rent);
        assert_eq!(accounts.len(), DECLARE_SUCCESSOR_ACCOUNT_COUNT_V1);
        accounts
    }

    fn runtime_plumbing(&self) -> (AccountInfo<'static>, AccountInfo<'static>) {
        let system = system_program_account();
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

    /// Everything the route decides before it creates the account.
    ///
    /// The creation is a System CPI that no unit-test runtime can serve, so the
    /// admitted path is exercised here and the created account is commit 10's
    /// real-SVM campaign. Every refusal below runs through the whole
    /// `process_instruction` entry point, because every one of them refuses
    /// before the CPI.
    fn compose(&self, accounts: &[AccountInfo<'static>]) -> Result<ReleaseLineageV1, ProgramError> {
        let predecessor_data = accounts
            .get(DECLARE_SUCCESSOR_PREDECESSOR_CACHE_ACCOUNT_V1)
            .expect("predecessor cache")
            .try_borrow_data()
            .expect("predecessor bytes");
        let successor_data = accounts
            .get(DECLARE_SUCCESSOR_SUCCESSOR_CACHE_ACCOUNT_V1)
            .expect("successor cache")
            .try_borrow_data()
            .expect("successor bytes");
        let predecessor = dclutch_registry_contract::ActivatedExecutionReleaseSetViewV1::decode(
            &predecessor_data,
        )
        .expect("predecessor cache decodes");
        let successor =
            dclutch_registry_contract::ActivatedExecutionReleaseSetViewV1::decode(&successor_data)
                .expect("successor cache decodes");
        let slot_at = |role: ExecutionRoleV1| {
            accounts
                .get(crate::lineage_v1::authority_account_index(role))
                .expect("consent slot")
        };
        let slots: [&AccountInfo<'static>; EXECUTION_ROLE_COUNT_V1] = [
            slot_at(ExecutionRoleV1::Core),
            slot_at(ExecutionRoleV1::Claims),
            slot_at(ExecutionRoleV1::Trading),
            slot_at(ExecutionRoleV1::Resolution),
            slot_at(ExecutionRoleV1::Custody),
        ];
        crate::lineage_v1::compose_lineage_for_test(predecessor, successor, &slots)
    }
}

fn declare(accounts: &[AccountInfo<'_>], registry: &Pubkey) -> ProgramResult {
    process_instruction(registry, accounts, &DeclareSuccessorV1::to_bytes())
}

fn refusal(result: ProgramResult) -> Option<u32> {
    match result {
        Err(ProgramError::Custom(code)) => Some(code),
        _ => None,
    }
}

#[test]
fn declaration_wire_is_argument_free_and_hostile_decoded() {
    let encoded = DeclareSuccessorV1::to_bytes();
    assert_eq!(encoded.len(), 16);
    assert_eq!(
        encoded.get(..8).expect("magic"),
        DECLARE_SUCCESSOR_MAGIC_V1.as_slice()
    );
    assert_eq!(DeclareSuccessorV1::decode(&encoded), Ok(DeclareSuccessorV1));

    // The magic must not collide with any wire the dispatcher routes first.
    for other in [
        dclutch_registry_svm::REGISTRY_INSTRUCTION_MAGIC_V1,
        dclutch_registry_svm::continuation_v1::REGISTRY_CONTINUATION_REQUEST_MAGIC_V1,
    ] {
        assert_ne!(DECLARE_SUCCESSOR_MAGIC_V1, other);
    }

    for (offset, expected) in [
        (0, dclutch_registry_svm::Error::InvalidMagic),
        (8, dclutch_registry_svm::Error::UnsupportedSchema),
        (10, dclutch_registry_svm::Error::NonCanonicalReservedBytes),
        (15, dclutch_registry_svm::Error::NonCanonicalReservedBytes),
    ] {
        let mut hostile = encoded;
        let byte = hostile.get_mut(offset).expect("hostile offset");
        *byte ^= 0xff;
        assert_eq!(DeclareSuccessorV1::decode(&hostile), Err(expected));
    }
    assert_eq!(
        DeclareSuccessorV1::decode(&encoded[..15]),
        Err(dclutch_registry_svm::Error::InvalidLength)
    );
}

#[test]
fn a_canonical_declaration_records_exactly_who_consented() {
    let fixture = LineageFixture::new();
    let moved = [true, false, true, false, false];
    let accounts = fixture.accounts(moved);
    let lineage = fixture.compose(&accounts).expect("canonical declaration");

    assert_eq!(lineage.predecessor(), fixture.predecessor_id);
    assert_eq!(lineage.successor(), fixture.successor_id);
    for role in EXECUTION_ROLE_ORDER_V1 {
        let index = role.role_index();
        let expected_move = copied(&moved, index);
        assert_eq!(
            lineage.moved(role),
            expected_move,
            "{role:?} moved verdict is derived, never supplied"
        );
        assert_eq!(
            lineage.consenting_authority(role),
            expected_move.then(|| copied(&fixture.authority, index).to_bytes()),
            "{role:?} consent slot"
        );
    }
}

#[test]
fn hostile_h2_forged_lineage_without_any_upgrade_authority() {
    // The attacker publishes a successor naming programs they do not control
    // and signs with their own key instead of the bound authority.
    let fixture = LineageFixture::new();
    let mut accounts = fixture.accounts([true, false, true, false, false]);
    let forger = account(
        Pubkey::new_from_array(bytes(0xde)),
        true,
        false,
        1,
        Vec::new(),
        system_program::ID,
        false,
    );
    let core_slot = crate::lineage_v1::authority_account_index(ExecutionRoleV1::Core);
    *accounts.get_mut(core_slot).expect("Core consent slot") = forger;
    assert_eq!(
        refusal(declare(&accounts, &fixture.registry)),
        Some(RegistryError::ReleaseLineageAuthorityMissing as u32)
    );
}

#[test]
fn hostile_h4_a_hop_may_not_change_a_role_program_id() {
    // The conjunct that keeps every child address in the protocol fixed.
    let mut fixture = LineageFixture::new();
    let sideways = lineage_role(
        Pubkey::new_from_array(bytes(0xc4)),
        0xc4,
        200,
        Some(copied(&fixture.authority, ExecutionRoleV1::Trading.role_index()).to_bytes()),
    );
    if let Some(slot) = fixture
        .successor
        .get_mut(ExecutionRoleV1::Trading.role_index())
    {
        *slot = sideways;
    }
    let accounts = fixture.accounts([true, false, true, false, false]);
    assert_eq!(
        refusal(declare(&accounts, &fixture.registry)),
        Some(RegistryError::ReleaseLineageRoleIdentityMoved as u32)
    );
}

#[test]
fn hostile_h5_a_declaration_cannot_run_backward_or_sideways_in_slot() {
    // Every other conjunct is satisfied — same programs, a real moved artifact,
    // the bound authority signing — so this reaches the slot comparison and
    // nothing earlier.
    for successor_slot in [100_u64, 99] {
        let mut fixture = LineageFixture::new();
        let index = ExecutionRoleV1::Core.role_index();
        let program = Pubkey::new_from_array(bytes(0x30));
        let backward = lineage_role(
            program,
            0x30 | 0x80,
            successor_slot,
            Some(copied(&fixture.authority, index).to_bytes()),
        );
        if let Some(slot) = fixture.successor.get_mut(index) {
            *slot = backward;
        }
        let accounts = fixture.accounts([true, false, true, false, false]);
        assert_eq!(
            refusal(declare(&accounts, &fixture.registry)),
            Some(RegistryError::ReleaseLineageNotForward as u32),
            "a successor at slot {successor_slot} is not forward of 100"
        );
    }
}

#[test]
fn hostile_h7_lineage_never_forks_because_the_account_is_keyed_by_predecessor() {
    let fixture = LineageFixture::new();
    let mut accounts = fixture.accounts([true, false, true, false, false]);
    // A first declaration already landed: the record exists, Registry-owned.
    let already = fixture.compose(&accounts).expect("first declaration");
    let occupied = account(
        release_lineage_address_v1(&fixture.registry, &fixture.predecessor_id.to_bytes()),
        false,
        true,
        fixture.rent.minimum_balance(RELEASE_LINEAGE_BYTES_V1),
        already.to_bytes().to_vec(),
        fixture.registry,
        false,
    );
    *accounts
        .get_mut(DECLARE_SUCCESSOR_LINEAGE_ACCOUNT_V1)
        .expect("lineage slot") = occupied;
    assert_eq!(
        refusal(declare(&accounts, &fixture.registry)),
        Some(RegistryError::ReleaseLineageAlreadyDeclared as u32)
    );
}

#[test]
fn hostile_h8_a_set_is_not_its_own_successor() {
    let fixture = LineageFixture::new();
    let mut accounts = fixture.accounts([true, false, true, false, false]);
    // Both caches name the same set, which is the only way to spell A -> A.
    *accounts
        .get_mut(DECLARE_SUCCESSOR_SUCCESSOR_CACHE_ACCOUNT_V1)
        .expect("successor cache") =
        fixture.cache_account(fixture.predecessor_id, fixture.predecessor);
    assert_eq!(
        refusal(declare(&accounts, &fixture.registry)),
        Some(RegistryError::ReleaseLineageSelfSuccession as u32)
    );
}

#[test]
fn hostile_h9_a_partially_activated_successor_cannot_be_declared() {
    // A market that migrated onto a half-activated set would land somewhere
    // inoperable. The cache's own decode is what forbids it.
    let fixture = LineageFixture::new();
    let mut accounts = fixture.accounts([true, false, true, false, false]);
    let partial = {
        let key = Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, fixture.successor_id.as_bytes()],
            &fixture.registry,
        )
        .0;
        let mut data = lineage_cache_bytes(fixture.successor_id, fixture.successor);
        // Two of five roles never admitted: exactly all-zero slots.
        let start = LINEAGE_CACHE_ROLES_OFFSET + 3 * ACTIVATED_ROLE_BYTES_V1;
        data.get_mut(start..).expect("tail roles").fill(0);
        account(
            key,
            false,
            false,
            fixture.rent.minimum_balance(data.len()),
            data,
            fixture.registry,
            false,
        )
    };
    *accounts
        .get_mut(DECLARE_SUCCESSOR_SUCCESSOR_CACHE_ACCOUNT_V1)
        .expect("successor cache") = partial;
    assert_eq!(
        refusal(declare(&accounts, &fixture.registry)),
        Some(RegistryError::ActivationCache as u32)
    );
}

#[test]
fn hostile_h10_an_unmoved_role_slot_cannot_carry_a_signature() {
    // Consent nobody asked for is still consent on the record, so an unmoved
    // slot must hold the System program and must not sign.
    let fixture = LineageFixture::new();

    let mut signing = fixture.accounts([true, false, true, false, false]);
    *signing
        .get_mut(crate::lineage_v1::authority_account_index(
            ExecutionRoleV1::Claims,
        ))
        .expect("Claims slot") = account(
        Pubkey::new_from_array(bytes(0xaa)),
        true,
        false,
        1,
        Vec::new(),
        system_program::ID,
        false,
    );
    assert_eq!(
        refusal(declare(&signing, &fixture.registry)),
        Some(RegistryError::ReleaseLineageAuthorityMissing as u32)
    );

    // And a non-signing stranger is refused too: the slot names one account.
    let mut stranger = fixture.accounts([true, false, true, false, false]);
    *stranger
        .get_mut(crate::lineage_v1::authority_account_index(
            ExecutionRoleV1::Claims,
        ))
        .expect("Claims slot") = account(
        Pubkey::new_from_array(bytes(0xab)),
        false,
        false,
        1,
        Vec::new(),
        system_program::ID,
        false,
    );
    assert_eq!(
        refusal(declare(&stranger, &fixture.registry)),
        Some(RegistryError::ReleaseLineageAuthorityMissing as u32)
    );
}

#[test]
fn hostile_h11_an_immutable_role_cannot_have_moved() {
    // An immutable deployment binds no authority, so "it moved" is not a
    // missing signature — it is a contradiction.
    let mut fixture = LineageFixture::new();
    let index = ExecutionRoleV1::Core.role_index();
    let program = Pubkey::new_from_array(bytes(0x30));
    if let Some(slot) = fixture.successor.get_mut(index) {
        *slot = lineage_role(program, 0x30 | 0x80, 200, None);
    }
    let accounts = fixture.accounts([true, false, true, false, false]);
    assert_eq!(
        refusal(declare(&accounts, &fixture.registry)),
        Some(RegistryError::ReleaseLineageAuthorityMissing as u32)
    );
}

#[test]
fn the_declaration_frame_is_exactly_eleven_accounts_with_tabled_privileges() {
    let fixture = LineageFixture::new();
    let moved = [true, false, true, false, false];
    let canonical = fixture.accounts(moved);
    // The canonical frame passes EVERY conjunct, up to and including the
    // pristine-account check, and stops only at the System creation itself.
    // This is what stops the hostiles below from being vacuous: each one is a
    // single departure from a frame that is otherwise admitted throughout.
    //
    // It is checked WITHOUT invoking the creation on purpose. A unit-test
    // `invoke_signed` mutates process-global syscall stubs and makes unrelated
    // tests in the same binary fail at random; the admitted path is proven here
    // and the creation itself is commit 10's real-SVM campaign.
    let lineage = fixture
        .compose(&canonical)
        .expect("the canonical frame passes conjuncts 2 through 6");
    assert_eq!(
        crate::lineage_v1::authenticate_pristine_lineage_account_for_test(
            &fixture.registry,
            canonical
                .get(DECLARE_SUCCESSOR_LINEAGE_ACCOUNT_V1)
                .expect("lineage account"),
            lineage,
        ),
        Ok(()).map(|()| {
            release_lineage_address_and_bump_v1(
                &fixture.registry,
                &fixture.predecessor_id.to_bytes(),
            )
            .1
        }),
        "the canonical frame's lineage account is pristine at its own address"
    );

    let mut short = canonical.clone();
    short.pop();
    assert_eq!(
        refusal(declare(&short, &fixture.registry)),
        Some(RegistryError::AccountFrame as u32)
    );
    let mut long = canonical.clone();
    long.push(canonical.last().expect("rent sysvar account").clone());
    assert_eq!(
        refusal(declare(&long, &fixture.registry)),
        Some(RegistryError::AccountFrame as u32)
    );

    // A writable cache is an account another instruction could still mutate.
    let mut writable_cache = canonical.clone();
    *writable_cache
        .get_mut(DECLARE_SUCCESSOR_PREDECESSOR_CACHE_ACCOUNT_V1)
        .expect("predecessor cache") = {
        let source = fixture.cache_account(fixture.predecessor_id, fixture.predecessor);
        account(
            *source.key,
            false,
            true,
            source.lamports(),
            source.try_borrow_data().expect("cache bytes").to_vec(),
            *source.owner,
            false,
        )
    };
    assert_eq!(
        refusal(declare(&writable_cache, &fixture.registry)),
        Some(RegistryError::AccountFrame as u32)
    );

    // A lineage account at someone else's address is not this declaration's.
    let mut wrong_address = canonical.clone();
    *wrong_address
        .get_mut(DECLARE_SUCCESSOR_LINEAGE_ACCOUNT_V1)
        .expect("lineage slot") = account(
        release_lineage_address_v1(&fixture.registry, &fixture.successor_id.to_bytes()),
        false,
        true,
        0,
        Vec::new(),
        system_program::ID,
        false,
    );
    assert_eq!(
        refusal(declare(&wrong_address, &fixture.registry)),
        Some(RegistryError::AccountFrame as u32)
    );
}

#[test]
fn a_consent_slot_admits_the_system_program_and_refuses_every_other_program() {
    // Conjunct 1's executable refusal keeps a program out of a consent slot: a
    // program holds no private key, so one standing here could only ever look
    // like a consent nothing was able to give. It has exactly one exception, the
    // account conjunct 6 itself REQUIRES for a role that did not move -- and
    // every runtime presents that account as executable.
    //
    // Both directions are checked, because an exemption exercised only in its
    // admitting direction is a hole nobody sees. The refusing direction is the
    // whole guarantee the exemption must not cost.
    let fixture = LineageFixture::new();
    let moved = [true, false, true, false, false];
    let unmoved_slot = crate::lineage_v1::authority_account_index(ExecutionRoleV1::Claims);
    let moved_slot = crate::lineage_v1::authority_account_index(ExecutionRoleV1::Core);

    // Admitting. Three of these five slots hold the real System Program account,
    // executable bit and all, and the frame is admitted.
    let canonical = fixture.accounts(moved);
    for role in [
        ExecutionRoleV1::Claims,
        ExecutionRoleV1::Resolution,
        ExecutionRoleV1::Custody,
    ] {
        let slot = canonical
            .get(crate::lineage_v1::authority_account_index(role))
            .expect("consent slot");
        assert_eq!(slot.key, &system_program::ID);
        assert!(
            slot.executable,
            "{role:?}'s unmoved slot holds the account a runtime presents, which is executable"
        );
    }
    assert_eq!(
        crate::lineage_v1::validate_declaration_frame_for_test(&canonical),
        Ok(()),
        "an unmoved role's slot is the one executable account this frame admits"
    );

    // Refusing. Any OTHER program in the same slot is refused, by name, before
    // anything is decoded -- including one owned by the loader that owns the
    // System Program, so the exemption is the KEY and not the pedigree.
    for (label, key, owner) in [
        (
            "a native program",
            Pubkey::new_from_array(bytes(0xc0)),
            native_loader::ID,
        ),
        (
            "a deployed program",
            Pubkey::new_from_array(bytes(0xc1)),
            bpf_loader_upgradeable::ID,
        ),
    ] {
        for slot in [unmoved_slot, moved_slot] {
            let mut smuggled = fixture.accounts(moved);
            *smuggled.get_mut(slot).expect("consent slot") = account(
                key,
                false,
                false,
                1,
                Vec::from(&b"a program"[..]),
                owner,
                true,
            );
            assert_eq!(
                crate::lineage_v1::validate_declaration_frame_for_test(&smuggled),
                Err(RegistryError::AccountFrame.into()),
                "{label} in a consent slot is still refused at the frame"
            );
            assert_eq!(
                refusal(declare(&smuggled, &fixture.registry)),
                Some(RegistryError::AccountFrame as u32),
                "{label} refuses through the whole route too"
            );
        }
    }

    // The exemption is the executable bit and nothing else: a WRITABLE consent
    // slot is refused whatever stands in it.
    let mut writable = fixture.accounts(moved);
    *writable.get_mut(unmoved_slot).expect("consent slot") = account(
        system_program::ID,
        false,
        true,
        1,
        Vec::from(&b"system_program"[..]),
        native_loader::ID,
        true,
    );
    assert_eq!(
        refusal(declare(&writable, &fixture.registry)),
        Some(RegistryError::AccountFrame as u32)
    );

    // And conjunct 1 conceded no decision by admitting the account: a MOVED
    // role's slot carrying it is refused by conjunct 6, which needs a signature
    // the System Program cannot produce. A role cannot be faked as unmoved.
    let mut faked = fixture.accounts(moved);
    *faked.get_mut(moved_slot).expect("consent slot") = system_program_account();
    assert_eq!(
        refusal(declare(&faked, &fixture.registry)),
        Some(RegistryError::ReleaseLineageAuthorityMissing as u32),
        "the System Program in a moved role's slot is not that role's consent"
    );
}

#[test]
fn the_declaration_reads_no_role_out_of_the_predecessor_cache() {
    // H15. The predecessor's cache is read for its bindings and its slots, and
    // for nothing that could ADMIT anything — a check that can only refuse is
    // not an exemption from a check. The proof is that the whole route still
    // composes when the predecessor's deployments are long superseded, which is
    // the exact state a stranded market is in.
    let mut fixture = LineageFixture::new();
    for role in EXECUTION_ROLE_ORDER_V1 {
        let index = role.role_index();
        let program = Pubkey::new_from_array(bytes(
            0x30 + u8::try_from(index).expect("role index is small"),
        ));
        let key = copied(&fixture.authority, index);
        // A predecessor whose recorded ELF and slot describe nothing deployed.
        if let Some(slot) = fixture.predecessor.get_mut(index) {
            *slot = lineage_role(
                program,
                0x30 + u8::try_from(index).expect("small"),
                1,
                Some(key.to_bytes()),
            );
        }
    }
    let accounts = fixture.accounts([true, false, true, false, false]);
    let lineage = fixture
        .compose(&accounts)
        .expect("a declaration does not care whether the predecessor still runs");
    assert_eq!(lineage.predecessor(), fixture.predecessor_id);
    assert_eq!(lineage.successor(), fixture.successor_id);
}

#[test]
fn every_role_moving_needs_every_authority_and_none_may_be_omitted() {
    // The five-of-five hop, and the proof that each slot is checked on its own
    // rather than any one signature standing for the set.
    let fixture = LineageFixture::with_moved([true; EXECUTION_ROLE_COUNT_V1]);
    let all = [true; EXECUTION_ROLE_COUNT_V1];
    let accounts = fixture.accounts(all);
    let lineage = fixture.compose(&accounts).expect("all five roles moved");
    for role in EXECUTION_ROLE_ORDER_V1 {
        assert!(lineage.moved(role), "{role:?} moved");
    }

    for role in EXECUTION_ROLE_ORDER_V1 {
        let mut missing = fixture.accounts(all);
        // Drop exactly one role's signature and keep everything else canonical.
        let index = role.role_index();
        *missing
            .get_mut(crate::lineage_v1::authority_account_index(role))
            .expect("consent slot") = account(
            copied(&fixture.authority, index),
            false,
            false,
            1,
            Vec::new(),
            system_program::ID,
            false,
        );
        assert_eq!(
            refusal(declare(&missing, &fixture.registry)),
            Some(RegistryError::ReleaseLineageAuthorityMissing as u32),
            "{role:?} must sign for its own move"
        );
    }
}
