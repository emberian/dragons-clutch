//! Adversarial cases for the CPI-free activation-cache read.
//!
//! Every child role adapter -- Claims, Custody, Core, Dealer and Rent -- reaches
//! the Registry-owned activation cache through the functions under test, so
//! these cases are the whole families' refusal set and not one family's.

#![allow(clippy::cast_possible_truncation)]

extern crate std;

use std::{boxed::Box, vec, vec::Vec};

use dclutch_core_contract::ContentId;
use crate::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ArtifactActivationInputV1, ArtifactUpgradePolicyV1,
    activate_execution_role_into_v1, initialize_activation_cache_v1,
};
use crate::release_set::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ProgramIdentityV1,
};

use solana_program::hash::hash;

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
    programdata_bytes(slot, None, elf)
}

/// The exact 45-byte Loader V3 ProgramData metadata span, then the ELF.
fn programdata_bytes(slot: u64, authority: Option<[u8; 32]>, elf: &[u8]) -> Vec<u8> {
    let mut output = vec![0_u8; 45 + elf.len()];
    output
        .get_mut(..4)
        .expect("variant")
        .copy_from_slice(&3_u32.to_le_bytes());
    output
        .get_mut(4..12)
        .expect("slot")
        .copy_from_slice(&slot.to_le_bytes());
    if let Some(authority) = authority {
        output
            .get_mut(12..13)
            .expect("option tag")
            .copy_from_slice(&[1]);
        output
            .get_mut(13..45)
            .expect("authority")
            .copy_from_slice(&authority);
    }
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
        Self::with_authority(seed, None)
    }

    /// The same whole activated release set over a MUTABLE deployment.
    ///
    /// Decision 0012: the release binds the exact authority and the exact
    /// deployment slot its activation observed, and every reader admits the
    /// cached digest only while both still hold.
    fn mutable(seed: u8, authority: [u8; 32]) -> Self {
        Self::with_authority(seed, Some(authority))
    }

    /// The next release generation over the SAME program id, re-pinned.
    ///
    /// This is what an operator publishes after upgrading the substrate: the
    /// same seven program ids, minted from the NEW chain observation.
    fn re_release(previous: &Self, authority: [u8; 32], elf: &[u8]) -> Self {
        Self::build(
            previous.program.key.to_bytes()[0],
            Some(authority),
            previous.slot + 1,
            elf.to_vec(),
        )
    }

    fn with_authority(seed: u8, authority: Option<[u8; 32]>) -> Self {
        Self::build(seed, authority, 77, vec![seed; 96])
    }

    fn build(seed: u8, authority: Option<[u8; 32]>, slot: u64, elf: Vec<u8>) -> Self {
        let registry = Pubkey::new_from_array([7; 32]);
        let role_program = Pubkey::new_from_array([seed; 32]);
        let programdata_key =
            Pubkey::find_program_address(&[role_program.as_ref()], &bpf_loader_upgradeable::ID).0;
        let release = ArtifactReleaseV1::new(
            ProgramIdentityV1::new(role_program.to_bytes()).expect("program"),
            ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader"),
            programdata_key.to_bytes(),
            ContentId::new([seed ^ 0x5a; 32]).expect("semantic release"),
            hash(&elf).to_bytes(),
            slot,
            match authority {
                None => ArtifactUpgradePolicyV1::Immutable,
                Some(_) => ArtifactUpgradePolicyV1::ExactAuthority,
            },
            authority,
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
            authority,
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
                programdata_bytes(slot, authority, &elf),
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

/// One canonical search can authenticate every role in the same cache; the
/// opaque bump reproduces byte-identical coordinates and a substituted bump
/// refuses before any cache byte is admitted.
#[test]
fn one_search_witness_reauthenticates_exact_cache_and_wrong_bump_refuses() {
    let fixture = Fixture::new(8);
    let registry = fixture.registry_account();
    let cache = fixture.cache();
    let release_set = fixture.release_set_id.to_bytes();
    let (first, bump) = authenticate_activated_role_and_bump_v1(
        &registry,
        &cache,
        &release_set,
        ExecutionRoleV1::Trading,
        &fixture.program,
        &fixture.programdata,
    )
    .expect("canonical search");
    let reused = authenticate_activated_role_with_bump_v1(
        &registry,
        &cache,
        &release_set,
        bump,
        ExecutionRoleV1::Trading,
        &fixture.program,
        &fixture.programdata,
    )
    .expect("create at authenticated bump");
    assert_eq!(first.to_bytes(), reused.to_bytes());

    let wrong = AuthenticatedActivationCacheBumpV1(bump.0.wrapping_sub(1));
    assert_eq!(
        authenticate_activated_role_with_bump_v1(
            &registry,
            &cache,
            &release_set,
            wrong,
            ExecutionRoleV1::Claims,
            &fixture.program,
            &fixture.programdata,
        ),
        Err(ActivationAuthErrorV1::ActivationCache),
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

/// Decision 0012 at the reader that every child role goes through.
///
/// This is the load-bearing case: the SAME code path Claims, Custody, Core,
/// Dealer and Rent all reach, and the Registry's own `Reauthenticate` handler,
/// admitting a MUTABLE deployment on its slot pin and refusing by name the
/// instant an `Upgrade` moves that slot. Nothing here hashes an ELF -- that is
/// the point of the pin, and it is what keeps the market life under the
/// 1,400,000 CU ceiling on a substrate the project can iterate.
#[test]
fn a_slot_pinned_mutable_deployment_authenticates_and_an_upgrade_supersedes_it() {
    let authority = [0x42_u8; 32];
    let fixture = Fixture::mutable(9, authority);
    let registry = fixture.registry_account();
    let cache = fixture.cache();

    for role in ALL_ROLES {
        let receipt = authenticate_activated_role_v1(
            &registry,
            &cache,
            &fixture.release_set_id.to_bytes(),
            role,
            &fixture.program,
            &fixture.programdata,
        )
        .expect("a mutable deployment authenticates while its pin holds");
        assert_eq!(receipt.role(), role);
    }

    // The upgrade lands. The Loader wrote a strictly later slot; the ELF bytes
    // here are deliberately UNCHANGED, so nothing but the slot can be doing the
    // refusing.
    let upgraded = account(
        *fixture.programdata.key,
        false,
        false,
        programdata_bytes(fixture.slot + 1, Some(authority), &fixture.elf),
        bpf_loader_upgradeable::ID,
        false,
    );
    for role in ALL_ROLES {
        assert_eq!(
            authenticate_activated_role_v1(
                &registry,
                &cache,
                &fixture.release_set_id.to_bytes(),
                role,
                &fixture.program,
                &upgraded,
            )
            .map(|_| ()),
            Err(ActivationAuthErrorV1::ReleaseSuperseded),
            "every dependent role refuses the moment the substrate is upgraded",
        );
    }

    // And with genuinely new bytes at the new slot -- the real upgrade shape.
    let replaced = account(
        *fixture.programdata.key,
        false,
        false,
        programdata_bytes(fixture.slot + 1, Some(authority), &[0x11_u8; 96]),
        bpf_loader_upgradeable::ID,
        false,
    );
    assert_eq!(
        authenticate_activated_role_v1(
            &registry,
            &cache,
            &fixture.release_set_id.to_bytes(),
            ExecutionRoleV1::Trading,
            &fixture.program,
            &replaced,
        )
        .map(|_| ()),
        Err(ActivationAuthErrorV1::ReleaseSuperseded),
    );
}

/// Every way OFF the pin that is not an upgrade keeps the generic refusal.
///
/// The operator-actionable name is reserved for the one event it describes.
/// A substituted authority, a revoked authority, and a slot BELOW the pin are
/// all "this is not the deployment I authenticated", not "you upgraded me".
#[test]
fn pin_substitution_is_refused_and_is_not_named_a_supersession() {
    let authority = [0x42_u8; 32];
    let fixture = Fixture::mutable(11, authority);
    let registry = fixture.registry_account();
    let cache = fixture.cache();

    let hostiles: [(&str, Vec<u8>); 4] = [
        (
            "a different upgrade authority at the pinned slot",
            programdata_bytes(fixture.slot, Some([0x43; 32]), &fixture.elf),
        ),
        (
            "an authority revoked out from under the release",
            programdata_bytes(fixture.slot, None, &fixture.elf),
        ),
        (
            "a slot below the pin, which no Loader write can produce",
            programdata_bytes(fixture.slot - 1, Some(authority), &fixture.elf),
        ),
        (
            "different bytes at a slot below the pin",
            programdata_bytes(fixture.slot - 1, Some(authority), &[0x11_u8; 96]),
        ),
    ];
    for (why, bytes) in hostiles {
        let hostile = account(
            *fixture.programdata.key,
            false,
            false,
            bytes,
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
                &hostile,
            )
            .map(|_| ()),
            Err(ActivationAuthErrorV1::Deployment),
            "{why}",
        );
    }
}

/// The same-slot redeploy edge: ProgramData identity participates in the pin.
///
/// Slot equality is only sound because it is checked against the ONE
/// ProgramData the Program account links to and the Loader derives. A hostile
/// account carrying a perfectly pinned slot, the exact bound authority and the
/// admitted ELF still refuses, because it is not that account.
#[test]
fn a_substituted_programdata_carrying_the_pinned_slot_still_refuses() {
    let authority = [0x42_u8; 32];
    let fixture = Fixture::mutable(13, authority);
    let registry = fixture.registry_account();
    let cache = fixture.cache();

    let impostor = account(
        Pubkey::new_from_array([0xee; 32]),
        false,
        false,
        programdata_bytes(fixture.slot, Some(authority), &fixture.elf),
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
            &impostor,
        )
        .map(|_| ()),
        Err(ActivationAuthErrorV1::Deployment),
    );
}

/// The re-release closes the loop: upgrade, refuse, re-pin, green.
///
/// A new release generation minted from the NEW observation authenticates
/// against the upgraded deployment, and the superseded generation stays
/// refused against it. That is the whole operator story of decision 0012, and
/// it holds without a single byte of the protocol changing between them.
#[test]
fn a_re_release_on_the_new_slot_authenticates_and_the_old_one_stays_refused() {
    let authority = [0x42_u8; 32];
    let superseded = Fixture::mutable(15, authority);
    let registry = superseded.registry_account();

    let upgraded_elf = vec![0x11_u8; 96];
    let upgraded = account(
        *superseded.programdata.key,
        false,
        false,
        programdata_bytes(superseded.slot + 1, Some(authority), &upgraded_elf),
        bpf_loader_upgradeable::ID,
        false,
    );
    assert_eq!(
        authenticate_activated_role_v1(
            &registry,
            &superseded.cache(),
            &superseded.release_set_id.to_bytes(),
            ExecutionRoleV1::Core,
            &superseded.program,
            &upgraded,
        )
        .map(|_| ()),
        Err(ActivationAuthErrorV1::ReleaseSuperseded),
    );

    let re_released = Fixture::re_release(&superseded, authority, &upgraded_elf);
    assert!(
        authenticate_activated_role_v1(
            &registry,
            &re_released.cache(),
            &re_released.release_set_id.to_bytes(),
            ExecutionRoleV1::Core,
            &re_released.program,
            &upgraded,
        )
        .is_ok(),
        "a release re-authenticated at the new slot admits the new deployment",
    );

    // And the re-released generation does not retroactively admit the old one.
    assert_eq!(
        authenticate_activated_role_v1(
            &registry,
            &re_released.cache(),
            &re_released.release_set_id.to_bytes(),
            ExecutionRoleV1::Core,
            &re_released.program,
            &superseded.programdata,
        )
        .map(|_| ()),
        Err(ActivationAuthErrorV1::Deployment),
    );
}

// ---------------------------------------------------------------------------
// One decode, several roles.
//
// Decision 0017's option B moved Trading's top-level arm off two Registry
// `Reauthenticate` CPIs and onto this crate. That arm reads FOUR roles out of
// one account, so the pair below -- `authenticate_activation_cache_identity_v1`
// then one `authenticate_activated_role_in_frame_v1` per role -- is the shape it
// takes, and these are its refusals.
//
// Every case here uses `MultiRoleFixture`, not `Fixture`. `Fixture` binds all
// five roles to the SAME program, which is legal and is what the aliasing
// branch of `validate_projection` exists to permit -- but it means a fold that
// read Core where it was asked for Trading would pass every assertion made
// against it. The distinct-program fixture is what makes the role INDEX a
// tested fact rather than an assumed one.
// ---------------------------------------------------------------------------

/// Five roles, five distinct programs, one activated release set.
struct MultiRoleFixture {
    registry: Pubkey,
    release_set_id: ContentId,
    cache_bytes: Vec<u8>,
    roles: [(ExecutionRoleV1, AccountInfo<'static>, AccountInfo<'static>); 5],
}

impl MultiRoleFixture {
    fn new() -> Self {
        let registry = Pubkey::new_from_array([7; 32]);
        let slot = 77_u64;
        let mut bindings = Vec::new();
        let mut inputs = Vec::new();
        let mut accounts = Vec::new();
        for (index, role) in ALL_ROLES.into_iter().enumerate() {
            // A distinct program id AND a distinct ELF per role, so no two
            // roles' releases, artifact ids or deployments can be confused.
            let seed = 0x20_u8.saturating_add(index as u8);
            let elf = vec![seed; 96 + index];
            let program_key = Pubkey::new_from_array([seed; 32]);
            let programdata_key =
                Pubkey::find_program_address(&[program_key.as_ref()], &bpf_loader_upgradeable::ID)
                    .0;
            let release = ArtifactReleaseV1::new(
                ProgramIdentityV1::new(program_key.to_bytes()).expect("program"),
                ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader"),
                programdata_key.to_bytes(),
                ContentId::new([seed ^ 0x5a; 32]).expect("semantic release"),
                hash(&elf).to_bytes(),
                slot,
                ArtifactUpgradePolicyV1::Immutable,
                None,
            )
            .expect("artifact release");
            let artifact_id = ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes())
                .expect("artifact id");
            bindings.push(ExecutionRoleBindingV1::new(release.program(), artifact_id));
            let observation = DeploymentObservationV1::new(
                program_key.to_bytes(),
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
            inputs.push(ArtifactActivationInputV1::new(
                artifact_id,
                release,
                observation,
            ));
            accounts.push((
                role,
                account(
                    program_key,
                    false,
                    false,
                    loader_program_bytes(programdata_key),
                    bpf_loader_upgradeable::ID,
                    true,
                ),
                account(
                    programdata_key,
                    false,
                    false,
                    immutable_programdata_bytes(slot, &elf),
                    bpf_loader_upgradeable::ID,
                    false,
                ),
            ));
        }
        let release_set = ExecutionReleaseSetV1::new(
            *bindings.first().expect("core"),
            *bindings.get(1).expect("claims"),
            *bindings.get(2).expect("trading"),
            *bindings.get(3).expect("resolution"),
            *bindings.get(4).expect("custody"),
        )
        .expect("five distinct roles");
        let release_set_id =
            ContentId::new(hash(&release_set.to_bytes()).to_bytes()).expect("release set id");
        let mut cache_bytes = vec![0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
        initialize_activation_cache_v1(&mut cache_bytes, release_set_id).expect("initialize");
        for (index, role) in ALL_ROLES.into_iter().enumerate() {
            activate_execution_role_into_v1(
                &mut cache_bytes,
                release_set_id,
                &release_set,
                role,
                inputs.get(index).expect("input"),
            )
            .expect("activate role");
        }
        let mut iterator = accounts.into_iter();
        let roles = [
            iterator.next().expect("core"),
            iterator.next().expect("claims"),
            iterator.next().expect("trading"),
            iterator.next().expect("resolution"),
            iterator.next().expect("custody"),
        ];
        Self {
            registry,
            release_set_id,
            cache_bytes,
            roles,
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

    fn cache(&self) -> AccountInfo<'static> {
        self.cache_at(
            activation_cache_address_v1(&self.registry, &self.release_set_id.to_bytes()),
            self.registry,
            self.cache_bytes.clone(),
        )
    }

    fn frame(&self, role: ExecutionRoleV1) -> (&AccountInfo<'static>, &AccountInfo<'static>) {
        let entry = self
            .roles
            .iter()
            .find(|(candidate, _, _)| *candidate == role)
            .expect("every role is staged");
        (&entry.1, &entry.2)
    }
}

/// Every role's program is distinct, so a role-index confusion cannot pass.
///
/// This is a property of the FIXTURE and it is asserted rather than assumed,
/// because the whole value of the multi-role cases below rests on it.
#[test]
fn the_multi_role_fixture_stages_five_distinct_programs() {
    let fixture = MultiRoleFixture::new();
    for (left_index, (_, left_program, left_programdata)) in fixture.roles.iter().enumerate() {
        for (_, right_program, right_programdata) in fixture.roles.iter().skip(left_index + 1) {
            assert_ne!(left_program.key, right_program.key);
            assert_ne!(left_programdata.key, right_programdata.key);
        }
    }
}

/// The whole point of the fold: N roles off ONE decode equal N independent
/// authentications, receipt byte for receipt byte.
///
/// If this ever goes red, the one-decode path is not the same authentication as
/// the per-role path and the CU saving was bought with a semantic.
#[test]
fn one_decode_authenticates_every_role_exactly_as_five_separate_reads_would() {
    let fixture = MultiRoleFixture::new();
    let registry = fixture.registry_account();
    let cache = fixture.cache();
    let release_set = fixture.release_set_id.to_bytes();

    let data = cache.try_borrow_data().expect("cache bytes");
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&data).expect("view");
    authenticate_activation_cache_identity_v1(&registry, &cache, &release_set, activated)
        .expect("the identity of the account the caller decoded");

    for role in ALL_ROLES {
        let (program, programdata) = fixture.frame(role);
        let folded =
            authenticate_activated_role_in_frame_v1(&cache, activated, role, program, programdata)
                .expect("role off the shared decode");
        let separate = authenticate_activated_role_v1(
            &registry,
            &cache,
            &release_set,
            role,
            program,
            programdata,
        )
        .expect("role off its own decode");
        assert_eq!(
            folded.to_bytes(),
            separate.to_bytes(),
            "{role:?} authenticated differently depending on who decoded the account",
        );
        assert_eq!(folded.role(), role);
        assert_eq!(folded.program().to_bytes(), program.key.to_bytes());
    }
}

/// Asking for one role and supplying another role's deployment refuses.
///
/// The aliased `Fixture` cannot state this case at all: with every role bound to
/// one program, every substitution is the identity. Here Core's Program and
/// ProgramData are handed to a Trading read, and
/// `cached_role_deployment_observation_v1` refuses at
/// `program.key != release.program()`.
#[test]
fn a_role_read_with_another_roles_deployment_refuses() {
    let fixture = MultiRoleFixture::new();
    let registry = fixture.registry_account();
    let cache = fixture.cache();
    let release_set = fixture.release_set_id.to_bytes();
    let (core_program, core_programdata) = fixture.frame(ExecutionRoleV1::Core);

    let data = cache.try_borrow_data().expect("cache bytes");
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&data).expect("view");
    authenticate_activation_cache_identity_v1(&registry, &cache, &release_set, activated)
        .expect("identity");

    assert_eq!(
        authenticate_activated_role_in_frame_v1(
            &cache,
            activated,
            ExecutionRoleV1::Trading,
            core_program,
            core_programdata,
        ),
        Err(ActivationAuthErrorV1::Deployment),
        "Core's deployment cannot answer for Trading's role, however valid it is",
    );
}

/// The identity check agrees with the borrow-and-decode entry it was extracted
/// from, bump for bump -- the drift gate on the extraction itself.
#[test]
fn the_identity_check_and_the_borrowing_entry_return_the_same_witness() {
    let fixture = MultiRoleFixture::new();
    let registry = fixture.registry_account();
    let cache = fixture.cache();
    let release_set = fixture.release_set_id.to_bytes();

    let borrowing = authenticate_activation_cache_bump_v1(&registry, &cache, &release_set)
        .expect("the borrowing entry");
    let data = cache.try_borrow_data().expect("cache bytes");
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&data).expect("view");
    let extracted =
        authenticate_activation_cache_identity_v1(&registry, &cache, &release_set, activated)
            .expect("the extracted identity check");
    assert_eq!(borrowing, extracted);

    // And the witness both produced still opens the cache for a role.
    let (program, programdata) = fixture.frame(ExecutionRoleV1::Custody);
    assert!(
        authenticate_activated_role_with_bump_v1(
            &registry,
            &cache,
            &release_set,
            extracted,
            ExecutionRoleV1::Custody,
            program,
            programdata,
        )
        .is_ok()
    );
}

/// A cache the Registry does not own is refused by the view-taking entry too.
///
/// The hazard the extraction has to answer: a caller that decodes for itself
/// could decode a STRANGER's bytes. It cannot get past this, because the
/// identity check re-takes ownership and width against the ACCOUNT and then
/// reproduces the address from the body's bump under the Registry.
#[test]
fn the_identity_check_refuses_a_foreign_owner_and_a_wrong_width() {
    let fixture = MultiRoleFixture::new();
    let registry = fixture.registry_account();
    let release_set = fixture.release_set_id.to_bytes();
    let address = activation_cache_address_v1(&fixture.registry, &release_set);

    let foreign = fixture.cache_at(
        address,
        Pubkey::new_from_array([200; 32]),
        fixture.cache_bytes.clone(),
    );
    let data = foreign.try_borrow_data().expect("cache bytes");
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&data).expect("view");
    assert_eq!(
        authenticate_activation_cache_identity_v1(&registry, &foreign, &release_set, activated),
        Err(ActivationAuthErrorV1::ActivationCache),
        "an account the Registry does not own cannot carry a Registry fact",
    );
    drop(data);

    // The width conjunct, stated as the hazard it actually answers.
    //
    // A wrong-width ACCOUNT cannot produce a view at all -- `decode` refuses it
    // on length -- so the naive substitution never reaches the identity check.
    // What does reach it is a caller who decodes a PREFIX: the first 1,288 bytes
    // of a longer Registry-owned account are a perfectly valid activation cache
    // body, and a view over them is indistinguishable from the real one. That is
    // the whole reason the identity check re-takes width against the ACCOUNT
    // rather than trusting the view it was handed.
    let mut widened = fixture.cache_bytes.clone();
    widened.push(0);
    let wide = fixture.cache_at(address, fixture.registry, widened);
    let wide_data = wide.try_borrow_data().expect("cache bytes");
    assert!(
        ActivatedExecutionReleaseSetViewV1::decode(&wide_data).is_err(),
        "a longer account is not decodable as the cache",
    );
    let prefix_view = ActivatedExecutionReleaseSetViewV1::decode(
        wide_data
            .get(..ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1)
            .expect("prefix"),
    )
    .expect("the prefix of a longer account decodes as a valid cache body");
    assert_eq!(
        authenticate_activation_cache_identity_v1(&registry, &wide, &release_set, prefix_view),
        Err(ActivationAuthErrorV1::ActivationCache),
        "the cache has exactly one width, and a view decoded from a prefix of a longer \
         account does not make that account the cache",
    );
}

/// A complete, valid cache for ANOTHER release set refuses at its address.
///
/// This is the conjunct the Registry CPI could not make. `authenticate_cache_identity`
/// derived the address from the release set the CACHE names, so this account
/// passed it, and what refused the transaction was the caller comparing the
/// returned receipt afterwards. Deriving from the release set the CALLER states
/// it is executing under refuses it here, before a role is decoded.
#[test]
fn a_valid_cache_for_another_generation_refuses_at_its_address_in_the_fold() {
    let mine = MultiRoleFixture::new();
    let theirs = Fixture::new(9);
    assert_ne!(
        mine.release_set_id.to_bytes(),
        theirs.release_set_id.to_bytes()
    );
    let registry = mine.registry_account();
    let theirs_cache = mine.cache_at(
        activation_cache_address_v1(&mine.registry, &theirs.release_set_id.to_bytes()),
        mine.registry,
        theirs.cache_bytes.clone(),
    );
    let data = theirs_cache.try_borrow_data().expect("cache bytes");
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&data).expect("view");
    assert_eq!(
        authenticate_activation_cache_identity_v1(
            &registry,
            &theirs_cache,
            &mine.release_set_id.to_bytes(),
            activated,
        ),
        Err(ActivationAuthErrorV1::ActivationCache),
        "another Market's activation is a real cache and is still not this one",
    );
}

/// The read-only frame is taken PER ROLE, not once for the cache.
///
/// Each role brings its own Program and ProgramData, so the frame is a statement
/// about the triple. A writable ProgramData on the second role must refuse even
/// though the first role's frame was clean and the cache is the right cache.
#[test]
fn a_writable_frame_on_the_second_role_refuses_after_the_first_succeeded() {
    let fixture = MultiRoleFixture::new();
    let registry = fixture.registry_account();
    let cache = fixture.cache();
    let release_set = fixture.release_set_id.to_bytes();
    let data = cache.try_borrow_data().expect("cache bytes");
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&data).expect("view");
    authenticate_activation_cache_identity_v1(&registry, &cache, &release_set, activated)
        .expect("identity");

    let (core_program, core_programdata) = fixture.frame(ExecutionRoleV1::Core);
    assert!(
        authenticate_activated_role_in_frame_v1(
            &cache,
            activated,
            ExecutionRoleV1::Core,
            core_program,
            core_programdata,
        )
        .is_ok(),
        "the first role's frame is clean",
    );

    let (trading_program, trading_programdata) = fixture.frame(ExecutionRoleV1::Trading);
    let writable = account(
        *trading_programdata.key,
        false,
        true,
        trading_programdata
            .try_borrow_data()
            .expect("programdata bytes")
            .to_vec(),
        *trading_programdata.owner,
        false,
    );
    assert_eq!(
        authenticate_activated_role_in_frame_v1(
            &cache,
            activated,
            ExecutionRoleV1::Trading,
            trading_program,
            &writable,
        ),
        Err(ActivationAuthErrorV1::AccountFrame),
        "a ProgramData another instruction can still mutate is not a deployment observation",
    );

    let signing_cache = account(
        *cache.key,
        true,
        false,
        fixture.cache_bytes.clone(),
        fixture.registry,
        false,
    );
    assert_eq!(
        authenticate_activated_role_in_frame_v1(
            &signing_cache,
            activated,
            ExecutionRoleV1::Trading,
            trading_program,
            trading_programdata,
        ),
        Err(ActivationAuthErrorV1::AccountFrame),
        "the cache's own privileges are re-taken on every role, not only the first",
    );
}

/// The fold is exactly the Registry's own handler body, per role.
///
/// `process_reauthenticate` runs `require_readonly_frame` and then
/// `authenticate_activated_role_in_cache_v1`. So does
/// `authenticate_activated_role_in_frame_v1`, and this is the assertion that
/// keeps it true: the drift decision 0017 §3 named as the shape's best property
/// is only structural while the two really are the same pair.
#[test]
fn the_frame_entry_is_the_registry_handler_pair_for_every_role() {
    let fixture = MultiRoleFixture::new();
    let cache = fixture.cache();
    let data = cache.try_borrow_data().expect("cache bytes");
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&data).expect("view");
    for role in ALL_ROLES {
        let (program, programdata) = fixture.frame(role);
        let handler = require_readonly_frame(&cache, program, programdata).and_then(|()| {
            authenticate_activated_role_in_cache_v1(activated, role, program, programdata)
        });
        let folded =
            authenticate_activated_role_in_frame_v1(&cache, activated, role, program, programdata);
        assert_eq!(
            handler.map(|receipt| receipt.to_bytes()),
            folded.map(|receipt| receipt.to_bytes()),
        );
    }
}
