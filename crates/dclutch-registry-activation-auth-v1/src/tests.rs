//! Adversarial cases for the CPI-free activation-cache read.
//!
//! Every child role adapter -- Claims, Custody, Core, Dealer and Rent -- reaches
//! the Registry-owned activation cache through the functions under test, so
//! these cases are the whole families' refusal set and not one family's.

extern crate std;

use std::{boxed::Box, vec, vec::Vec};

use dclutch_core_contract::ContentId;
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ArtifactActivationInputV1, ArtifactUpgradePolicyV1,
    activate_execution_role_into_v1, initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ProgramIdentityV1,
};

use super::*;

const ALL_ROLES: [ExecutionRoleV1; 5] = [
    ExecutionRoleV1::Core,
    ExecutionRoleV1::Claims,
    ExecutionRoleV1::Trading,
    ExecutionRoleV1::Resolution,
    ExecutionRoleV1::Custody,
];

fn account(
    key: Pubkey,
    signer: bool,
    writable: bool,
    data: Vec<u8>,
    owner: Pubkey,
    executable: bool,
) -> AccountInfo<'static> {
    AccountInfo::new(
        Box::leak(Box::new(key)),
        signer,
        writable,
        Box::leak(Box::new(1_u64)),
        Box::leak(data.into_boxed_slice()),
        Box::leak(Box::new(owner)),
        executable,
    )
}

fn loader_program_bytes(programdata: Pubkey) -> Vec<u8> {
    let mut output = vec![0_u8; 36];
    output
        .get_mut(..4)
        .expect("variant")
        .copy_from_slice(&2_u32.to_le_bytes());
    output
        .get_mut(4..36)
        .expect("link")
        .copy_from_slice(programdata.as_ref());
    output
}

fn immutable_programdata_bytes(slot: u64, elf: &[u8]) -> Vec<u8> {
    let mut output = vec![0_u8; 45 + elf.len()];
    output
        .get_mut(..4)
        .expect("variant")
        .copy_from_slice(&3_u32.to_le_bytes());
    output
        .get_mut(4..12)
        .expect("slot")
        .copy_from_slice(&slot.to_le_bytes());
    output.get_mut(45..).expect("elf").copy_from_slice(elf);
    output
}

/// One whole activated release set, exactly as the Registry writes it.
struct Fixture {
    registry: Pubkey,
    release_set_id: ContentId,
    artifact_id: ArtifactReleaseIdV1,
    release: ArtifactReleaseV1,
    cache_bytes: Vec<u8>,
    program: AccountInfo<'static>,
    programdata: AccountInfo<'static>,
    slot: u64,
    elf: Vec<u8>,
}

impl Fixture {
    fn new(seed: u8) -> Self {
        let registry = Pubkey::new_from_array([7; 32]);
        let role_program = Pubkey::new_from_array([seed; 32]);
        let programdata_key =
            Pubkey::find_program_address(&[role_program.as_ref()], &bpf_loader_upgradeable::ID).0;
        let elf = vec![seed; 96];
        let slot = 77;
        let release = ArtifactReleaseV1::new(
            ProgramIdentityV1::new(role_program.to_bytes()).expect("program"),
            ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader"),
            programdata_key.to_bytes(),
            ContentId::new([seed ^ 0x5a; 32]).expect("semantic release"),
            hash(&elf).to_bytes(),
            slot,
            ArtifactUpgradePolicyV1::Immutable,
            None,
        )
        .expect("artifact release");
        let artifact_id =
            ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes()).expect("artifact id");
        let binding = ExecutionRoleBindingV1::new(release.program(), artifact_id);
        let release_set = ExecutionReleaseSetV1::new(binding, binding, binding, binding, binding)
            .expect("aliased release set");
        let release_set_id =
            ContentId::new(hash(&release_set.to_bytes()).to_bytes()).expect("release set id");
        let observation = DeploymentObservationV1::new(
            role_program.to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            true,
            programdata_key.to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            false,
            programdata_key.to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            slot,
            hash(&elf).to_bytes(),
            None,
        )
        .expect("observation");
        let input = ArtifactActivationInputV1::new(artifact_id, release, observation);
        let mut cache_bytes = vec![0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
        initialize_activation_cache_v1(&mut cache_bytes, release_set_id).expect("initialize");
        for role in ALL_ROLES {
            activate_execution_role_into_v1(
                &mut cache_bytes,
                release_set_id,
                &release_set,
                role,
                &input,
            )
            .expect("activate role");
        }
        Self {
            registry,
            release_set_id,
            artifact_id,
            release,
            cache_bytes,
            program: account(
                role_program,
                false,
                false,
                loader_program_bytes(programdata_key),
                bpf_loader_upgradeable::ID,
                true,
            ),
            programdata: account(
                programdata_key,
                false,
                false,
                immutable_programdata_bytes(slot, &elf),
                bpf_loader_upgradeable::ID,
                false,
            ),
            slot,
            elf,
        }
    }

    fn registry_account(&self) -> AccountInfo<'static> {
        account(
            self.registry,
            false,
            false,
            Vec::new(),
            native_loader(),
            true,
        )
    }

    fn cache_at(&self, key: Pubkey, owner: Pubkey, bytes: Vec<u8>) -> AccountInfo<'static> {
        account(key, false, false, bytes, owner, false)
    }

    /// The exact account the Registry opened for this release set.
    fn cache(&self) -> AccountInfo<'static> {
        self.cache_at(
            activation_cache_address_v1(&self.registry, &self.release_set_id.to_bytes()),
            self.registry,
            self.cache_bytes.clone(),
        )
    }
}

fn native_loader() -> Pubkey {
    Pubkey::new_from_array(solana_sdk_ids::native_loader::ID.to_bytes())
}

/// This is the case the ninth wall refused as reentrancy.
///
/// A child at CPI depth three cannot invoke a Registry that is already at depth
/// one. Reading the cache it already carries produces the same receipt the
/// refused invocation would have returned.
#[test]
fn the_reentrant_case_now_succeeds_as_a_cache_read() {
    let fixture = Fixture::new(8);
    let registry = fixture.registry_account();
    let cache = fixture.cache();
    let receipt = authenticate_activated_role_v1(
        &registry,
        &cache,
        &fixture.release_set_id.to_bytes(),
        ExecutionRoleV1::Claims,
        &fixture.program,
        &fixture.programdata,
    )
    .expect("the frame already carries every fact the CPI would have returned");
    assert_eq!(receipt.role(), ExecutionRoleV1::Claims);
    assert_eq!(
        receipt.execution_release_set_id().to_bytes(),
        fixture.release_set_id.to_bytes()
    );
    assert_eq!(receipt.program().to_bytes(), fixture.program.key.to_bytes());
    assert_eq!(
        receipt.artifact_release_id().to_bytes(),
        fixture.artifact_id.to_bytes()
    );
    assert_eq!(
        receipt.semantic_release_id().to_bytes(),
        fixture.release.semantic_release_id().to_bytes()
    );
}

/// The Registry's own handler and the child-local read are the same function.
#[test]
fn the_registry_handler_and_the_child_read_agree_byte_for_byte() {
    let fixture = Fixture::new(8);
    let registry = fixture.registry_account();
    let cache = fixture.cache();
    for role in ALL_ROLES {
        let local = authenticate_activated_role_v1(
            &registry,
            &cache,
            &fixture.release_set_id.to_bytes(),
            role,
            &fixture.program,
            &fixture.programdata,
        )
        .expect("child-local read");
        let data = cache.try_borrow_data().expect("cache bytes");
        let view = ActivatedExecutionReleaseSetViewV1::decode(&data).expect("view");
        let handler = authenticate_activated_role_in_cache_v1(
            view,
            role,
            &fixture.program,
            &fixture.programdata,
        )
        .expect("Registry handler body");
        assert_eq!(local.to_bytes(), handler.to_bytes());
    }
}

/// A substituted cache account carrying the correct bytes at the wrong address.
#[test]
fn a_substituted_cache_account_refuses() {
    let fixture = Fixture::new(8);
    let registry = fixture.registry_account();
    let substituted = fixture.cache_at(
        Pubkey::new_from_array([99; 32]),
        fixture.registry,
        fixture.cache_bytes.clone(),
    );
    assert_eq!(
        authenticate_activated_role_v1(
            &registry,
            &substituted,
            &fixture.release_set_id.to_bytes(),
            ExecutionRoleV1::Core,
            &fixture.program,
            &fixture.programdata,
        ),
        Err(ActivationAuthErrorV1::ActivationCache),
        "correct bytes at an address the Registry never derived are not the cache",
    );
}

/// A cache at the right address owned by anything but the Registry.
#[test]
fn a_cache_owned_by_anyone_but_the_registry_refuses() {
    let fixture = Fixture::new(8);
    let registry = fixture.registry_account();
    let foreign_owner = fixture.cache_at(
        activation_cache_address_v1(&fixture.registry, &fixture.release_set_id.to_bytes()),
        Pubkey::new_from_array([200; 32]),
        fixture.cache_bytes.clone(),
    );
    assert_eq!(
        authenticate_activated_role_v1(
            &registry,
            &foreign_owner,
            &fixture.release_set_id.to_bytes(),
            ExecutionRoleV1::Core,
            &fixture.program,
            &fixture.programdata,
        ),
        Err(ActivationAuthErrorV1::ActivationCache),
        "an account the Registry does not own cannot carry a Registry fact",
    );
}

/// A foreign release set: the caller names one activation, the cache is another.
///
/// This is also the cache-for-another-Market case. The second fixture is a
/// COMPLETE, VALID activation cache -- the Registry really opened it, for a
/// different release set -- and it is refused at its address, before a byte of
/// it is read, because the address is derived from the generation the caller
/// states it is executing under.
#[test]
fn a_valid_cache_for_another_release_set_refuses_at_its_address() {
    let mine = Fixture::new(8);
    let theirs = Fixture::new(9);
    assert_ne!(
        mine.release_set_id.to_bytes(),
        theirs.release_set_id.to_bytes()
    );
    let registry = mine.registry_account();
    assert_eq!(
        authenticate_activated_role_v1(
            &registry,
            &theirs.cache(),
            &mine.release_set_id.to_bytes(),
            ExecutionRoleV1::Core,
            &theirs.program,
            &theirs.programdata,
        ),
        Err(ActivationAuthErrorV1::ActivationCache),
        "another Market's activation is not this action's activation",
    );
    // And the symmetric direction: the right cache, the wrong stated generation.
    assert_eq!(
        authenticate_activated_role_v1(
            &registry,
            &mine.cache(),
            &theirs.release_set_id.to_bytes(),
            ExecutionRoleV1::Core,
            &mine.program,
            &mine.programdata,
        ),
        Err(ActivationAuthErrorV1::ActivationCache),
    );
}

/// A cache PLACED at the right address whose header names another generation.
///
/// The address check alone cannot catch this -- the account is where it is
/// supposed to be -- so the header's own `execution_release_set_id` is compared
/// as well, and that is the check the retired CPI never made on the caller's
/// behalf.
#[test]
fn a_cache_whose_header_names_another_generation_refuses() {
    let mine = Fixture::new(8);
    let theirs = Fixture::new(9);
    let registry = mine.registry_account();
    let forged = mine.cache_at(
        activation_cache_address_v1(&mine.registry, &mine.release_set_id.to_bytes()),
        mine.registry,
        theirs.cache_bytes.clone(),
    );
    assert_eq!(
        authenticate_activated_role_v1(
            &registry,
            &forged,
            &mine.release_set_id.to_bytes(),
            ExecutionRoleV1::Core,
            &theirs.program,
            &theirs.programdata,
        ),
        Err(ActivationAuthErrorV1::ActivationCache),
    );
}

/// A deployment that moved after activation admitted it.
#[test]
fn a_redeployed_role_refuses_on_its_current_deployment() {
    let fixture = Fixture::new(8);
    let registry = fixture.registry_account();
    let cache = fixture.cache();
    let stale = account(
        *fixture.programdata.key,
        false,
        false,
        immutable_programdata_bytes(fixture.slot + 1, &fixture.elf),
        bpf_loader_upgradeable::ID,
        false,
    );
    assert_eq!(
        authenticate_activated_role_v1(
            &registry,
            &cache,
            &fixture.release_set_id.to_bytes(),
            ExecutionRoleV1::Core,
            &fixture.program,
            &stale,
        ),
        Err(ActivationAuthErrorV1::Deployment),
        "a slot the activation did not admit is not the admitted deployment",
    );

    let other_elf = vec![0x11_u8; 96];
    let rewritten = account(
        *fixture.programdata.key,
        false,
        false,
        immutable_programdata_bytes(fixture.slot, &other_elf),
        bpf_loader_upgradeable::ID,
        false,
    );
    // An immutable release reuses its activation-bound ELF digest rather than
    // re-hashing, and the ProgramData carrying different bytes at the same slot
    // is not observable here -- the deployment SLOT and the absent upgrade
    // authority are what make the rewrite impossible on chain. The observation
    // still authenticates, which is the fast path's stated argument, and this
    // case pins that it is the argument being relied on.
    assert!(
        authenticate_activated_role_v1(
            &registry,
            &cache,
            &fixture.release_set_id.to_bytes(),
            ExecutionRoleV1::Core,
            &fixture.program,
            &rewritten,
        )
        .is_ok()
    );
}

/// A frame that would lend privileges the read-only route never has.
#[test]
fn a_writable_or_signing_frame_refuses() {
    let fixture = Fixture::new(8);
    let registry = fixture.registry_account();
    let writable_cache = fixture.cache_at(
        activation_cache_address_v1(&fixture.registry, &fixture.release_set_id.to_bytes()),
        fixture.registry,
        fixture.cache_bytes.clone(),
    );
    let mut writable_cache = writable_cache;
    writable_cache.is_writable = true;
    assert_eq!(
        authenticate_activated_role_v1(
            &registry,
            &writable_cache,
            &fixture.release_set_id.to_bytes(),
            ExecutionRoleV1::Core,
            &fixture.program,
            &fixture.programdata,
        ),
        Err(ActivationAuthErrorV1::AccountFrame),
    );

    let mut signing_program = fixture.program.clone();
    signing_program.is_signer = true;
    assert_eq!(
        authenticate_activated_role_v1(
            &registry,
            &fixture.cache(),
            &fixture.release_set_id.to_bytes(),
            ExecutionRoleV1::Core,
            &signing_program,
            &fixture.programdata,
        ),
        Err(ActivationAuthErrorV1::AccountFrame),
    );
}

/// A role's Program substituted for another executable account.
#[test]
fn a_program_the_activation_did_not_name_refuses() {
    let mine = Fixture::new(8);
    let theirs = Fixture::new(9);
    let registry = mine.registry_account();
    assert_eq!(
        authenticate_activated_role_v1(
            &registry,
            &mine.cache(),
            &mine.release_set_id.to_bytes(),
            ExecutionRoleV1::Core,
            &theirs.program,
            &theirs.programdata,
        ),
        Err(ActivationAuthErrorV1::Deployment),
    );
}
