//! The builder's own tests, shaped like the hop the cut will actually declare.
//!
//! Every fixture cache here is produced by `build_registry_activation_v1` and
//! not by hand-assembled bytes, so a cache these tests admit is one the
//! activation builder would have written. The default hop is the real 7→8 shape
//! recovered from devnet: four roles moved, resolution unmoved, and all five
//! roles binding one shared upgrade authority.

use dclutch_core_contract::ContentId;
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::release_set::{
    ArtifactReleaseIdV1, EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1, ExecutionReleaseSetV1,
    ExecutionRoleBindingV1, ProgramIdentityV1,
};
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1,
};
use solana_program::{account_info::AccountInfo, hash::hash, rent::Rent, sysvar::SysvarSerialize};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, sysvar};

use super::*;
use crate::registry::{
    RegistryActivationState, RegistryFinalizedRecordState, RegistryRoleSetState, RegistryRoleState,
    build_registry_activation_v1,
};
use crate::{Finality, Observation};

const REGISTRY_SEED: u8 = 7;
const PAYER_SEED: u8 = 90;
const AUTHORITY_SEED: u8 = 44;
/// Deliberately not the one every role binds, so a test that swaps it proves
/// the projection followed the cache rather than a constant.
const OTHER_AUTHORITY_SEED: u8 = 45;

fn seeded(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

fn observation() -> Observation {
    Observation {
        slot: 491_018_122,
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

/// Loader V3 ProgramData: variant, slot, then the 33-byte authority option.
fn programdata_bytes(slot: u64, authority: Option<Pubkey>, elf: &[u8]) -> Vec<u8> {
    let mut output = vec![0; 45 + elf.len()];
    output
        .get_mut(..4)
        .expect("variant")
        .copy_from_slice(&3_u32.to_le_bytes());
    output
        .get_mut(4..12)
        .expect("slot")
        .copy_from_slice(&slot.to_le_bytes());
    if let Some(authority) = authority {
        *output.get_mut(12).expect("tag") = 1;
        output
            .get_mut(13..45)
            .expect("authority")
            .copy_from_slice(authority.as_ref());
    }
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

fn rent_account(rent: &Rent) -> ObservedAccount {
    let mut lamports = 1;
    let mut data = vec![0; Rent::size_of()];
    let key = sysvar::rent::ID;
    let owner = sysvar::ID;
    let mut info = AccountInfo::new(&key, false, false, &mut lamports, &mut data, &owner, false);
    assert_eq!(rent.clone().to_account_info(&mut info), Some(()));
    observed(key, owner, 1, false, data)
}

fn payer_account() -> ObservedAccount {
    observed(
        seeded(PAYER_SEED),
        system_program::ID,
        2_645_351_216,
        false,
        Vec::new(),
    )
}

fn system_account() -> ObservedAccount {
    observed(system_program::ID, native_loader::ID, 1, true, Vec::new())
}

/// One role's deployment inside a release set, as the cut would observe it.
#[derive(Clone, Copy)]
struct RoleSpec {
    /// Fixed across a hop by conjunct 4; varied only to prove it refuses.
    program_seed: u8,
    /// Changing this is exactly what "the artifact moved" means.
    elf_seed: u8,
    slot: u64,
    authority: Option<u8>,
}

impl RoleSpec {
    const fn at(program_seed: u8, elf_seed: u8, slot: u64) -> Self {
        Self {
            program_seed,
            elf_seed,
            slot,
            authority: Some(AUTHORITY_SEED),
        }
    }
}

/// Cohort-7, as the five caches recovered from devnet are shaped.
fn predecessor_specs() -> [RoleSpec; 5] {
    [
        RoleSpec::at(11, 101, 490_697_000),
        RoleSpec::at(12, 102, 490_697_100),
        RoleSpec::at(13, 103, 490_697_200),
        RoleSpec::at(14, 104, 490_693_331),
        RoleSpec::at(15, 105, 490_697_400),
    ]
}

/// Rebind the upgrade authority on the four roles that move, and ONLY those.
///
/// The unmoved role has to stay byte-identical, and the authority is part of
/// those bytes: rebinding it there would change the artifact release id, which
/// is the sole definition of "moved" — and the role would then be a moved one
/// whose deployment slot did not advance, refused by conjunct 5. The tests
/// below that vary an authority are varying consent, not movement, so they must
/// leave the unmoved role alone to vary only the coordinate under test.
fn rebind_moved_authorities(mut specs: [RoleSpec; 5], seed: u8) -> [RoleSpec; 5] {
    for (index, spec) in specs.iter_mut().enumerate() {
        if index != ExecutionRoleV1::Resolution.role_index() {
            spec.authority = Some(seed);
        }
    }
    specs
}

/// Cohort-8: core, claims, trading and custody moved; resolution did not.
fn successor_specs() -> [RoleSpec; 5] {
    [
        RoleSpec::at(11, 201, 490_849_793),
        RoleSpec::at(12, 202, 490_826_560),
        RoleSpec::at(13, 203, 490_830_840),
        // Byte-identical to the predecessor's: this is the unmoved role.
        RoleSpec::at(14, 104, 490_693_331),
        RoleSpec::at(15, 205, 490_814_947),
    ]
}

struct Cache {
    release_set_id: ContentId,
    account: ObservedAccount,
}

/// Build one complete activation cache through the activation builder itself.
fn build_cache(registry: Pubkey, rent: &Rent, specs: [RoleSpec; 5]) -> Cache {
    let mut roles = Vec::with_capacity(5);
    let mut bindings = Vec::with_capacity(5);
    for spec in specs {
        let program = seeded(spec.program_seed);
        let programdata =
            Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0;
        let elf = vec![spec.elf_seed; 96];
        let authority = spec.authority.map(seeded);
        let release = ArtifactReleaseV1::new(
            ProgramIdentityV1::new(program.to_bytes()).expect("program"),
            ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader"),
            programdata.to_bytes(),
            ContentId::new([spec.program_seed; 32]).expect("semantic release"),
            hash(&elf).to_bytes(),
            spec.slot,
            match authority {
                Some(_) => ArtifactUpgradePolicyV1::ExactAuthority,
                None => ArtifactUpgradePolicyV1::Immutable,
            },
            authority.map(|key| key.to_bytes()),
        )
        .expect("release");
        let (artifact_release, artifact_digest) = finalized_record(
            registry,
            ARTIFACT_RELEASE_SCHEMA_ID_V1,
            release.to_bytes().to_vec(),
            rent,
        );
        bindings.push(ExecutionRoleBindingV1::new(
            release.program(),
            ArtifactReleaseIdV1::new(artifact_digest).expect("artifact"),
        ));
        roles.push(RegistryRoleState {
            artifact_release,
            program: observed(
                program,
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
                programdata_bytes(spec.slot, authority, &elf),
            ),
        });
    }
    let [core, claims, trading, resolution, custody]: [ExecutionRoleBindingV1; 5] =
        bindings.try_into().ok().expect("five bindings");
    let release_set = ExecutionReleaseSetV1::new(core, claims, trading, resolution, custody)
        .expect("release set");
    let (execution_release_set, release_set_digest) = finalized_record(
        registry,
        EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1,
        release_set.to_bytes().to_vec(),
        rent,
    );
    let release_set_id = ContentId::new(release_set_digest).expect("release set id");
    let cache_key = activation_cache_address_v1(&registry, release_set_id.as_bytes());
    let [core, claims, trading, resolution, custody]: [RegistryRoleState; 5] =
        roles.try_into().ok().expect("five roles");
    let state = RegistryActivationState {
        payer: payer_account(),
        cache: observed(cache_key, system_program::ID, 0, false, Vec::new()),
        execution_release_set,
        roles: RegistryRoleSetState {
            core,
            claims,
            trading,
            resolution,
            custody,
        },
        system_program: system_account(),
        rent_sysvar: rent_account(rent),
    };
    let report = build_registry_activation_v1(registry, &state).expect("activation");
    Cache {
        release_set_id,
        account: observed(
            cache_key,
            registry,
            rent.minimum_balance(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1),
            false,
            report.expected_cache.to_bytes().to_vec(),
        ),
    }
}

struct Fixture {
    registry: Pubkey,
    rent: Rent,
    predecessor: Cache,
    successor: Cache,
    state: RegistryDeclareSuccessorState,
}

impl Fixture {
    fn with(predecessor_specs: [RoleSpec; 5], successor_specs: [RoleSpec; 5]) -> Self {
        let registry = seeded(REGISTRY_SEED);
        let rent = Rent::default();
        let predecessor = build_cache(registry, &rent, predecessor_specs);
        let successor = build_cache(registry, &rent, successor_specs);
        let lineage =
            release_lineage_address_and_bump_v1(&registry, predecessor.release_set_id.as_bytes()).0;
        let state = RegistryDeclareSuccessorState {
            payer: payer_account(),
            lineage: observed(lineage, system_program::ID, 0, false, Vec::new()),
            predecessor_cache: predecessor.account.clone(),
            successor_cache: successor.account.clone(),
            system_program: system_account(),
            rent_sysvar: rent_account(&rent),
        };
        Self {
            registry,
            rent,
            predecessor,
            successor,
            state,
        }
    }

    fn new() -> Self {
        Self::with(predecessor_specs(), successor_specs())
    }

    fn build(&self) -> Result<RegistryDeclareSuccessorReport, Error> {
        build_registry_declare_successor_v1(self.registry, &self.state)
    }

    fn report(&self) -> RegistryDeclareSuccessorReport {
        self.build().expect("declaration")
    }
}

#[test]
fn the_devnet_shaped_hop_projects_four_signing_slots_and_one_system_slot() {
    let fixture = Fixture::new();
    let report = fixture.report();
    let authority = seeded(AUTHORITY_SEED);

    assert_eq!(report.instruction.program_id, fixture.registry);
    assert_eq!(
        report.instruction.accounts.len(),
        DECLARE_SUCCESSOR_ACCOUNT_COUNT_V1
    );
    assert_eq!(report.instruction.data, DeclareSuccessorV1::to_bytes());
    assert_eq!(report.predecessor, fixture.predecessor.release_set_id);
    assert_eq!(report.successor, fixture.successor.release_set_id);

    let accounts = &report.instruction.accounts;
    let payer = accounts.first().expect("payer");
    assert_eq!(payer.pubkey, fixture.state.payer.key);
    assert!(payer.is_signer && payer.is_writable);
    let lineage = accounts.get(1).expect("lineage");
    assert_eq!(lineage.pubkey, report.lineage);
    assert!(!lineage.is_signer && lineage.is_writable);
    for index in 2..4 {
        let cache = accounts.get(index).expect("cache");
        assert!(!cache.is_signer && !cache.is_writable);
    }

    // Four roles moved and each demands the cache-bound authority's signature;
    // resolution did not move, so its slot holds the System Program and must
    // NOT sign. That asymmetry is the whole of conjunct 6 in one frame.
    for role in EXECUTION_ROLE_ORDER_V1 {
        let index = DECLARE_SUCCESSOR_AUTHORITY_BASE_ACCOUNT_V1 + role.role_index();
        let slot = accounts.get(index).expect("consent slot");
        let projected = report.consent.get(role.role_index()).expect("projection");
        assert_eq!(projected.role, role);
        assert!(!slot.is_writable);
        if role == ExecutionRoleV1::Resolution {
            assert!(!projected.moved);
            assert_eq!(slot.pubkey, system_program::ID);
            assert!(!slot.is_signer);
            assert_eq!(report.record.consenting_authority(role), None);
        } else {
            assert!(projected.moved);
            assert_eq!(slot.pubkey, authority);
            assert!(slot.is_signer);
            assert_eq!(
                report.record.consenting_authority(role),
                Some(authority.to_bytes())
            );
        }
        assert!(report.record.moved(role) == projected.moved);
    }

    let system = accounts.get(9).expect("system");
    assert_eq!(system.pubkey, system_program::ID);
    assert!(!system.is_signer && !system.is_writable);
    let rent = accounts.get(10).expect("rent");
    assert_eq!(rent.pubkey, sysvar::rent::ID);
    assert!(!rent.is_signer && !rent.is_writable);

    // Five roles, one shared deployer: the frame needs two signatures, not six.
    assert_eq!(
        report.required_signers,
        vec![fixture.state.payer.key, authority]
    );
    assert_eq!(
        report.lineage_rent_debit_lamports,
        fixture.rent.minimum_balance(RELEASE_LINEAGE_BYTES_V1)
    );
    assert_eq!(report.record.to_bytes().len(), RELEASE_LINEAGE_BYTES_V1);
    assert_eq!(
        ReleaseLineageV1::decode(&report.record.to_bytes()),
        Ok(report.record)
    );
}

/// O-016: the consenting key is read out of the successor cache, so it moves
/// when the cache moves and there is no caller input that could hold it still.
#[test]
fn the_consenting_key_follows_the_successor_cache_and_no_caller_input() {
    let specs = rebind_moved_authorities(successor_specs(), OTHER_AUTHORITY_SEED);
    let fixture = Fixture::with(predecessor_specs(), specs);
    let report = fixture.report();
    let other = seeded(OTHER_AUTHORITY_SEED);

    // The state this builder was handed is byte-identical in every field a
    // caller controls; only the cache's own bytes changed, and the frame did.
    assert_eq!(
        report.required_signers,
        vec![fixture.state.payer.key, other]
    );
    for role in EXECUTION_ROLE_ORDER_V1 {
        let projected = report.consent.get(role.role_index()).expect("projection");
        if role == ExecutionRoleV1::Resolution {
            assert_eq!(projected.slot, system_program::ID);
        } else {
            assert_eq!(projected.slot, other);
            assert_ne!(projected.slot, seeded(AUTHORITY_SEED));
        }
    }
}

/// The predecessor's cache binds an authority too, and it is NOT the one asked.
/// Consent is the successor's claim to make, so the predecessor's bindings can
/// never reach a consent slot.
#[test]
fn consent_is_read_from_the_successor_never_the_predecessor() {
    let predecessor = rebind_moved_authorities(predecessor_specs(), OTHER_AUTHORITY_SEED);
    let fixture = Fixture::with(predecessor, successor_specs());
    let report = fixture.report();
    assert_eq!(
        report.required_signers,
        vec![fixture.state.payer.key, seeded(AUTHORITY_SEED)]
    );
}

#[test]
fn the_frame_address_helper_agrees_with_the_built_frame() {
    let fixture = Fixture::new();
    let report = fixture.report();
    let addresses = declare_successor_frame_addresses_v1(
        fixture.registry,
        fixture.predecessor.release_set_id.as_bytes(),
        fixture.successor.release_set_id.as_bytes(),
    );
    assert_eq!(addresses.lineage, report.lineage);
    assert_eq!(addresses.lineage_bump, report.lineage_bump);
    assert_eq!(
        addresses.predecessor_cache,
        fixture.state.predecessor_cache.key
    );
    assert_eq!(addresses.successor_cache, fixture.state.successor_cache.key);
}

#[test]
fn an_already_declared_predecessor_refuses_before_a_frame_is_built() {
    let mut fixture = Fixture::new();
    fixture.state.lineage.owner = fixture.registry;
    fixture.state.lineage.lamports = 2_616_960;
    fixture.state.lineage.data = vec![0; RELEASE_LINEAGE_BYTES_V1];
    assert_eq!(fixture.build(), Err(Error::LineageAlreadyDeclared));
}

#[test]
fn a_lineage_account_at_any_other_address_refuses() {
    let mut fixture = Fixture::new();
    fixture.state.lineage.key = seeded(200);
    assert_eq!(fixture.build(), Err(Error::InvalidLineageAddress));
}

#[test]
fn a_hop_from_a_set_to_itself_refuses() {
    let mut fixture = Fixture::new();
    fixture.state.successor_cache = fixture.state.predecessor_cache.clone();
    assert_eq!(fixture.build(), Err(Error::LineageSelfSuccession));
}

#[test]
fn a_hop_that_moves_a_role_program_identity_refuses() {
    let mut successor = successor_specs();
    if let Some(spec) = successor.first_mut() {
        spec.program_seed = 99;
    }
    let fixture = Fixture::with(predecessor_specs(), successor);
    assert_eq!(fixture.build(), Err(Error::LineageRoleIdentityMoved));
}

#[test]
fn a_moved_role_whose_slot_did_not_advance_refuses() {
    let mut successor = successor_specs();
    if let Some(spec) = successor.first_mut() {
        // New bytes, older slot: an upgrade that ran backwards.
        spec.slot = 1;
    }
    let fixture = Fixture::with(predecessor_specs(), successor);
    assert_eq!(fixture.build(), Err(Error::LineageNotForward));
}

/// An `Immutable` artifact binds no authority, so a hop claiming it moved is a
/// contradiction rather than a missing signature.
#[test]
fn a_moved_role_that_binds_no_authority_refuses() {
    let mut successor = successor_specs();
    if let Some(spec) = successor.first_mut() {
        spec.authority = None;
    }
    let fixture = Fixture::with(predecessor_specs(), successor);
    assert_eq!(fixture.build(), Err(Error::LineageAuthorityMissing));
}

#[test]
fn a_cache_that_is_not_the_registrys_own_at_its_derived_address_refuses() {
    let mut fixture = Fixture::new();
    fixture.state.successor_cache.key = seeded(201);
    assert_eq!(fixture.build(), Err(Error::InvalidActivationCache));

    let mut fixture = Fixture::new();
    fixture.state.predecessor_cache.owner = seeded(202);
    assert_eq!(fixture.build(), Err(Error::InvalidActivationCache));
}

#[test]
fn an_unfinalized_or_split_observation_refuses_before_anything_is_decoded() {
    let mut fixture = Fixture::new();
    fixture.state.successor_cache.observation.finality = Finality::Confirmed;
    assert_eq!(fixture.build(), Err(Error::ObservationNotFinalized));

    let mut fixture = Fixture::new();
    fixture.state.successor_cache.observation.slot = 491_018_123;
    assert_eq!(fixture.build(), Err(Error::ObservationMismatch));
}

#[test]
fn a_payer_that_cannot_cover_the_record_refuses() {
    let mut fixture = Fixture::new();
    fixture.state.payer.lamports = 1;
    assert_eq!(fixture.build(), Err(Error::InsufficientPayer));
}

/// The record carries no clock, so the hop's bytes do not depend on when it is
/// authored. Two builds at observations two months apart must be byte-equal.
#[test]
fn a_hop_authored_late_composes_the_same_bytes_as_a_timely_one() {
    let fixture = Fixture::new();
    let timely = fixture.report();

    let mut later = Fixture::new();
    for account in [
        &mut later.state.payer,
        &mut later.state.lineage,
        &mut later.state.predecessor_cache,
        &mut later.state.successor_cache,
        &mut later.state.system_program,
        &mut later.state.rent_sysvar,
    ] {
        account.observation.slot = 512_000_000;
        account.observation.unix_timestamp = 1_805_000_000;
    }
    let late = later.report();

    assert_eq!(timely.record.to_bytes(), late.record.to_bytes());
    assert_eq!(timely.instruction, late.instruction);
}
