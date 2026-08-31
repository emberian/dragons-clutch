//! The real-ELF campaign for `InitializeProtocolInfrastructureV2`, driving the
//! compiled Core.
//!
//! `docs/design/PROFILE_UPGRADE_RULING_2026_08_31.md` §8 item 2 — "the ceremony
//! admits exactly the ceremony" — and the byte-identity half of item 3: the
//! full §5 conjunct set green once, then one hostile per conjunct, then the
//! predecessor V1 account proved byte-identical across all of it.
//!
//! The honest succession is composed by the SHIPPED host builder
//! (`dclutch_operator::infrastructure_succession_v1`), never by hand. What is
//! actually at risk here is not whether the program is correct — its unit suite
//! and the builder's own suite both say so — but whether the only tool that can
//! call it produces a frame the twenty-one-account `parse` accepts. A campaign
//! that hand-assembled its own frame would prove the program right and leave the
//! builder untested.
//!
//! # What is proved
//!
//! * The builder's frame lands on the compiled program, and the 224 bytes it
//!   projected locally are byte-for-byte the bytes that end up at the V2 PDA
//!   (conjunct 7's read-back belt, observed from outside).
//! * The V1 profile account is byte-identical before and after everything —
//!   the write-once/CloseSeal bar. V1 is read as evidence and never written.
//! * Each of conjuncts 2 through 6 refuses under its OWN name, with the frame
//!   perturbed in exactly one place or the world doctored in exactly one place.
//!   Anti-vacuity is the point: LINEAGE-WRITER's campaign caught two hostiles
//!   that landed what they meant to refuse.
//!
//! Conjunct 7 has no hostile reachable from outside the program: the persisted
//! image is written by `create_profile_v2` itself and read back in the same
//! invocation, so nothing a caller can present doctors it. It is proved
//! positively instead, by byte-comparing what landed against what the builder
//! projected. Doctoring it belongs to the ruling's mutation floor (§8.4).
//!
//! # One deployment generation, and why that is enough
//!
//! `solana-program-test` holds exactly one nonzero deployment generation per
//! bank (`dclutch-trading-sbf/program-test/direct-hot/src/waist.rs`, on
//! `observed_deployment_slot`), so Core, Registry and Rent are all planted at
//! [`SUCCESSOR_DEPLOYMENT_SLOT`] and the bank runs one slot later. The
//! succession does not need a second generation: the predecessor artifact
//! record is content-pinned and its deployment is deliberately never observed
//! (`infrastructure_v2.rs`, on `authenticate_predecessor_record`), so
//! [`PREDECESSOR_DEPLOYMENT_SLOT`] exists only inside that record's bytes and no
//! account is planted for it.
//!
//! Run: `programs/dclutch-core-sbf/run-open-market-program-test.sh`

use std::{env, fs, path::PathBuf, vec, vec::Vec};

use dclutch_core_contract::ContentId;
use dclutch_core_sbf::{
    CoreSbfError, INITIALIZE_INFRASTRUCTURE_ACCOUNT_COUNT_V2 as CORE_FRAME_ACCOUNT_COUNT_V2,
};
use dclutch_operator::{
    Finality, Observation, ObservedAccount,
    infrastructure_succession_v1::{
        CoreInfrastructureSuccessionReportV1, CoreInfrastructureSuccessionStateV1,
        Error as SuccessionBuilderError, INITIALIZE_INFRASTRUCTURE_ACCOUNT_COUNT_V2,
        InfrastructureBindingV1, PredecessorRecordObservationV1, REGISTRY_CONSENT_ACCOUNT_V2,
        RENT_CONSENT_ACCOUNT_V2, build_core_infrastructure_succession_v1,
    },
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1, ArtifactUpgradePolicyV1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionRoleBindingV1, PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2, PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2, ProgramIdentityV1,
    ProtocolInfrastructureProfileV1, ProtocolInfrastructureProfileV2,
};
use solana_account::Account;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program::{
    clock::Clock,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::{
    instruction::InstructionError,
    signature::{Keypair, Signer},
    transaction::TransactionError,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::Transaction;

/// The builder RESTATES the succession frame's width instead of importing it —
/// a host builder crate does not link the Core program, and `dclutch-operator`
/// cannot depend on `dclutch-core-sbf` without a cycle. This campaign is the
/// only compilation unit that sees both numbers, so it is the only place they
/// can be held to each other; nothing else in the tree would notice them drift.
const _: () = assert!(
    CORE_FRAME_ACCOUNT_COUNT_V2 == INITIALIZE_INFRASTRUCTURE_ACCOUNT_COUNT_V2,
    "the builder's restated succession frame width drifted from the program's"
);

/// Where the compiled Core is deployed for this campaign.
///
/// Any address works — both profile domains are derived UNDER it — and it is
/// deliberately not the devnet Core, because nothing here should read as a
/// claim about devnet.
const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xe1; 32]);
/// Where the compiled Registry is deployed. Every artifact record in this
/// campaign, the Rent record included, lives under it.
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xe2; 32]);
/// Where the compiled Rent program is deployed.
const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xe3; 32]);
/// A Registry program identity that is deployed nowhere.
///
/// Conjunct 3 compares the successor record's program against the one V1 named,
/// so a V1 profile naming this identity is the whole of the relocation hostile;
/// no deployment behind it is needed, or wanted.
const RELOCATED_REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xe4; 32]);
/// An address this fixture never plants an account at.
///
/// The runtime presents it as the System-owned zero-lamport empty account, which
/// is exactly what an absent predecessor profile looks like.
const UNPLANTED_ADDRESS: Pubkey = Pubkey::new_from_array([0xd0; 32]);

/// The single deployment generation every planted ProgramData reports.
const SUCCESSOR_DEPLOYMENT_SLOT: u64 = 531;
/// The slot the bank executes at: one past the deployment, the Loader V3
/// delay-visibility rule, and the only slot at which a nonzero generation is
/// both effective and rooted under `warp_to_slot`.
const BANK_SLOT: u64 = SUCCESSOR_DEPLOYMENT_SLOT + 1;
/// The slot the predecessor Registry record binds.
///
/// It appears only inside that record's bytes. The ceremony reads the record
/// for its slot and its bound authority and never observes the deployment
/// behind it, so no account is planted here and none is needed.
const PREDECESSOR_DEPLOYMENT_SLOT: u64 = 167;
/// A predecessor deployment slot that does not precede the successor's.
///
/// Conjunct 4 wants strictly forward, so binding a predecessor record LATER than
/// the live deployment is an upgrade claimed to have run backwards.
const NON_ADVANCING_PREDECESSOR_SLOT: u64 = SUCCESSOR_DEPLOYMENT_SLOT + 69;

/// A predecessor record binding the SAME slot the live deployment reports.
///
/// The boundary conjunct 4 actually defends. A record binding a later slot is
/// refused by any comparison that rejects going backwards; only an equal slot
/// separates "strictly forward" from "forward or standing still", and standing
/// still is a succession that re-selects the deployment it already had under a
/// different record. The mutation floor found this: relaxing the route's `<=`
/// to `<` survived the campaign until this case existed.
const EQUAL_PREDECESSOR_SLOT: u64 = SUCCESSOR_DEPLOYMENT_SLOT;

/// The compute ceiling the ceremony runs under.
///
/// The moved Registry binding is a first admission, so the complete deployed
/// Registry ELF is hashed on chain; the unmoved Rent binding rides V1's
/// admission and is not.
const PROTOCOL_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;
/// The ceremony's index inside every transaction here: each leads with an
/// explicit compute-unit limit, so a program refusal is reported against 1.
const CEREMONY_INSTRUCTION_INDEX: u8 = 1;

/// Frame index of the V1 profile presented as conjunct 2's evidence.
const PREDECESSOR_PROFILE_ACCOUNT: usize = 2;
/// Frame index of Core's live ProgramData upgrade-authority signer.
const CORE_UPGRADE_AUTHORITY_ACCOUNT: usize = 4;
/// Frame index of the moved Registry binding's predecessor record bytes.
const PREDECESSOR_REGISTRY_RAW_ACCOUNT: usize = 13;
/// Frame index of that record's staging cursor.
const PREDECESSOR_REGISTRY_STAGING_ACCOUNT: usize = 14;

/// Core's Loader V3 upgrade authority.
///
/// Deliberately NOT the key the infrastructure records bind. The frame permits
/// that aliasing (slots 0, 4, 15 and 18 are natural-person slots), but a shared
/// key would make the unsigned-authority hostile vacuous: one key appearing
/// twice in one message carries the union of its privileges, so clearing the
/// signer bit on slot 4 while the same key signs at slot 15 changes nothing at
/// all and the frame would LAND what it meant to refuse.
fn core_authority() -> Keypair {
    Keypair::new_from_array([0x41; 32])
}

/// The key the Registry's predecessor record binds, and the one conjunct 5
/// demands a signature from. It is also what the live Registry and Rent
/// ProgramData accounts carry, which is what the successor records pin.
fn consent_authority() -> Keypair {
    Keypair::new_from_array([0x42; 32])
}

/// A funded, signing key that is neither of the above.
fn intruder() -> Keypair {
    Keypair::new_from_array([0x43; 32])
}

/// Funds the V2 profile, and is NOT the transaction fee payer.
///
/// The frame's payer slot must be a writable signer and a consent slot may not
/// be writable, so a deployer key that also funded the ceremony would compose a
/// message no runtime can serve — the builder refuses that pairing by name.
/// Keeping the frame payer off the fee-payer seat additionally keeps the
/// writable bit a real coordinate rather than one the message compiler grants
/// unconditionally to message index 0.
fn funder() -> Keypair {
    Keypair::new_from_array([0x2f; 32])
}

/// The three real links this campaign deploys.
struct Artifacts {
    core: Vec<u8>,
    registry: Vec<u8>,
    rent: Vec<u8>,
}

fn artifacts() -> Artifacts {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    let artifacts = Artifacts {
        core: fs::read(directory.join("dclutch_core_sbf.so")).expect("Core ELF"),
        registry: fs::read(directory.join("dclutch_registry_sbf.so")).expect("Registry ELF"),
        rent: fs::read(directory.join("dclutch_rent_sbf.so")).expect("Rent ELF"),
    };
    for (name, elf) in [
        ("Core", &artifacts.core),
        ("Registry", &artifacts.registry),
        ("Rent", &artifacts.rent),
    ] {
        assert_eq!(
            elf.get(..4),
            Some(&[0x7f, b'E', b'L', b'F'][..]),
            "the {name} artifact must be a real ELF"
        );
    }
    artifacts
}

fn programdata_address(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

/// Loader V3 ProgramData: variant tag, deployment slot, the 33-byte authority
/// option, then the complete ELF tail.
fn programdata_bytes(elf: &[u8], authority: Pubkey) -> Vec<u8> {
    let mut bytes = vec![0; 45 + elf.len()];
    bytes
        .get_mut(0..4)
        .expect("tag")
        .copy_from_slice(&3_u32.to_le_bytes());
    bytes
        .get_mut(4..12)
        .expect("slot")
        .copy_from_slice(&SUCCESSOR_DEPLOYMENT_SLOT.to_le_bytes());
    *bytes.get_mut(12).expect("authority tag") = 1;
    bytes
        .get_mut(13..45)
        .expect("authority")
        .copy_from_slice(authority.as_ref());
    bytes.get_mut(45..).expect("ELF").copy_from_slice(elf);
    bytes
}

/// Deploy one program at [`SUCCESSOR_DEPLOYMENT_SLOT`] under `authority`.
///
/// The genesis helper writes a ProgramData reporting slot 0; the override that
/// follows is what pins the generation, and `ProgramTest::add_account` stores
/// after genesis so it wins.
fn add_program(
    test: &mut ProgramTest,
    name: &'static str,
    program: Pubkey,
    elf: &[u8],
    authority: Pubkey,
) {
    test.add_upgradeable_program_to_genesis(name, &program);
    let data = programdata_bytes(elf, authority);
    test.add_account(
        programdata_address(program),
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: bpf_loader_upgradeable::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn wallet(test: &mut ProgramTest, key: Pubkey, lamports: u64) {
    test.add_account(
        key,
        Account {
            lamports,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn identity(program: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(program.to_bytes()).expect("program identity")
}

/// One slot-pinned `ExactAuthority` release, bound to `consent_authority`.
///
/// Both live infrastructure deployments carry that key, so a successor record's
/// pin authenticates against what is actually deployed; a predecessor record
/// carries it because conjunct 5 reads the consenting signer out of exactly
/// there.
fn artifact_release(
    program: Pubkey,
    elf_digest: [u8; 32],
    semantic: u8,
    deployment_slot: u64,
) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        identity(program),
        identity(bpf_loader_upgradeable::ID),
        programdata_address(program).to_bytes(),
        ContentId::new([semantic; 32]).expect("semantic release"),
        elf_digest,
        deployment_slot,
        ArtifactUpgradePolicyV1::ExactAuthority,
        Some(consent_authority().pubkey().to_bytes()),
    )
    .expect("slot-pinned artifact release")
}

/// One finalized artifact record: its bytes, its two canonical PDAs, and the
/// binding a profile pins it by.
#[derive(Clone, Copy)]
struct Record {
    raw: Pubkey,
    staging: Pubkey,
    binding: ExecutionRoleBindingV1,
}

/// Plant one Registry-owned finalized record and its vacant staging cursor.
fn add_artifact_record(test: &mut ProgramTest, release: ArtifactReleaseV1) -> Record {
    let data = release.to_bytes().to_vec();
    let digest = hash(&data).to_bytes();
    let raw = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &digest,
        ],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    let staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &digest,
        ],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    test.add_account(
        raw,
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: REGISTRY_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    // Nonzero lamports on a vacant cursor are unclassified dust to the record
    // authority; ownership and emptiness are the finalized-absence evidence.
    test.add_account(
        staging,
        Account {
            lamports: 1,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    Record {
        raw,
        staging,
        binding: ExecutionRoleBindingV1::new(
            release.program(),
            ArtifactReleaseIdV1::new(digest).expect("artifact release id"),
        ),
    }
}

/// Which V1 profile a bank is planted with.
///
/// Every other account is identical across all four worlds — the program ids
/// are constants and each record's address is its own digest — so the honest
/// twenty-one-account frame is the SAME frame in every one of them. Only the
/// 144 bytes at the V1 PDA move.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorldV1 {
    /// The ruling's cohort-9 shape: the Registry moved from
    /// [`PREDECESSOR_DEPLOYMENT_SLOT`] to [`SUCCESSOR_DEPLOYMENT_SLOT`], and the
    /// Rent program did not move at all. The unmoved binding is the coordinate
    /// that makes conjunct 5 asymmetric, and it is real, not decorative.
    RegistryMoved,
    /// V1 named a Registry program identity that is not the deployed one.
    RegistryRelocated,
    /// V1 already pins both successor records: the succession selects nothing.
    NothingMoved,
    /// V1 pins a Registry record whose deployment slot is LATER than the live
    /// one, so the claimed upgrade ran backwards.
    PredecessorAhead,
    /// V1 pins a Registry record binding the SAME slot the live deployment
    /// reports, so the succession advances nowhere.
    PredecessorLevel,
}

/// One planted world, and the addresses its campaign reads.
struct Fixture {
    profile_v1: Pubkey,
    profile_v2: Pubkey,
    /// The Registry record V1 pins in the cohort-9 shape, at
    /// [`PREDECESSOR_DEPLOYMENT_SLOT`].
    predecessor_registry: Record,
    /// The Registry record V1 pins in [`WorldV1::PredecessorAhead`].
    non_advancing_registry: Record,
    /// The Registry record V1 pins in [`WorldV1::PredecessorLevel`].
    level_registry: Record,
    registry: Record,
    rent: Record,
    /// The V1 profile the cohort-9 world plants, and the one the builder is
    /// handed in every world.
    honest_v1_profile: ProtocolInfrastructureProfileV1,
    /// The Registry record THIS world's V1 profile pins, presented as conjunct
    /// 4 and 5's evidence whenever the bytes say the binding moved.
    pinned_registry: Record,
    /// Whether this world's bytes say the Registry binding moved.
    registry_moved: bool,
    core_authority: Keypair,
    consent: Keypair,
    funder: Keypair,
}

impl Fixture {
    fn new(world: WorldV1) -> (ProgramTest, Self) {
        let artifacts = artifacts();
        let core_authority = core_authority();
        let consent = consent_authority();
        let funder = funder();

        let mut test = ProgramTest::default();
        test.prefer_bpf(true);
        test.set_compute_max_units(u64::from(PROTOCOL_COMPUTE_UNIT_LIMIT));
        // The bank has to BE one slot past the single deployment generation for
        // any of these programs to load, and the Clock the runtime serves must
        // agree with the bank it is serving.
        test.add_sysvar_account(
            sysvar::clock::ID,
            &Clock {
                slot: BANK_SLOT,
                ..Clock::default()
            },
        );
        add_program(
            &mut test,
            "dclutch_core_sbf",
            CORE_PROGRAM_ID,
            &artifacts.core,
            core_authority.pubkey(),
        );
        add_program(
            &mut test,
            "dclutch_registry_sbf",
            REGISTRY_PROGRAM_ID,
            &artifacts.registry,
            consent.pubkey(),
        );
        add_program(
            &mut test,
            "dclutch_rent_sbf",
            RENT_PROGRAM_ID,
            &artifacts.rent,
            consent.pubkey(),
        );
        wallet(&mut test, funder.pubkey(), 10_000_000_000);
        wallet(&mut test, core_authority.pubkey(), 1_000_000_000);
        wallet(&mut test, consent.pubkey(), 1_000_000_000);
        wallet(&mut test, intruder().pubkey(), 1_000_000_000);

        // The two selections the succession makes, each pinned to the ELF
        // actually planted in ProgramData: the ceremony hashes the complete
        // observed ELF on the moved arm and re-proves the pin on the unmoved
        // one, so a record claiming other bytes is refused rather than trusted.
        let registry = add_artifact_record(
            &mut test,
            artifact_release(
                REGISTRY_PROGRAM_ID,
                hash(&artifacts.registry).to_bytes(),
                0xa1,
                SUCCESSOR_DEPLOYMENT_SLOT,
            ),
        );
        let rent = add_artifact_record(
            &mut test,
            artifact_release(
                RENT_PROGRAM_ID,
                hash(&artifacts.rent).to_bytes(),
                0xa2,
                SUCCESSOR_DEPLOYMENT_SLOT,
            ),
        );
        // The superseded Registry records. Their ELF digests describe bytes that
        // are deployed nowhere, which is exactly what a superseded record is:
        // the ceremony reads each for its slot and its bound authority and never
        // observes a deployment behind it.
        let predecessor_registry = add_artifact_record(
            &mut test,
            artifact_release(
                REGISTRY_PROGRAM_ID,
                [0xb1; 32],
                0xb1,
                PREDECESSOR_DEPLOYMENT_SLOT,
            ),
        );
        let non_advancing_registry = add_artifact_record(
            &mut test,
            artifact_release(
                REGISTRY_PROGRAM_ID,
                [0xb2; 32],
                0xb2,
                NON_ADVANCING_PREDECESSOR_SLOT,
            ),
        );
        let level_registry = add_artifact_record(
            &mut test,
            artifact_release(
                REGISTRY_PROGRAM_ID,
                [0xb3; 32],
                0xb3,
                EQUAL_PREDECESSOR_SLOT,
            ),
        );

        let honest_v1_profile =
            ProtocolInfrastructureProfileV1::new(predecessor_registry.binding, rent.binding)
                .expect("cohort-9 V1 profile");
        let (pinned_registry, planted_v1_profile) = match world {
            WorldV1::RegistryMoved => (predecessor_registry, honest_v1_profile),
            WorldV1::RegistryRelocated => (
                predecessor_registry,
                ProtocolInfrastructureProfileV1::new(
                    ExecutionRoleBindingV1::new(
                        identity(RELOCATED_REGISTRY_PROGRAM_ID),
                        predecessor_registry.binding.artifact_release(),
                    ),
                    rent.binding,
                )
                .expect("relocated V1 profile"),
            ),
            WorldV1::NothingMoved => (
                registry,
                ProtocolInfrastructureProfileV1::new(registry.binding, rent.binding)
                    .expect("already-current V1 profile"),
            ),
            WorldV1::PredecessorAhead => (
                non_advancing_registry,
                ProtocolInfrastructureProfileV1::new(non_advancing_registry.binding, rent.binding)
                    .expect("non-advancing V1 profile"),
            ),
            WorldV1::PredecessorLevel => (
                level_registry,
                ProtocolInfrastructureProfileV1::new(level_registry.binding, rent.binding)
                    .expect("level V1 profile"),
            ),
        };

        let profile_v1 = Pubkey::find_program_address(
            &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
            &CORE_PROGRAM_ID,
        )
        .0;
        let profile_v2 = Pubkey::find_program_address(
            &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
            &CORE_PROGRAM_ID,
        )
        .0;
        test.add_account(
            profile_v1,
            Account {
                lamports: Rent::default().minimum_balance(PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1),
                data: planted_v1_profile.to_bytes().to_vec(),
                owner: CORE_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        );
        // The one vacancy this domain will ever have, planted exactly as the V1
        // ceremony plants its own: System-owned, dataless, and carrying dust the
        // route tops up rather than refuses.
        test.add_account(
            profile_v2,
            Account {
                lamports: 1,
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            },
        );

        let registry_moved =
            pinned_registry.binding.artifact_release() != registry.binding.artifact_release();
        (
            test,
            Self {
                profile_v1,
                profile_v2,
                predecessor_registry,
                non_advancing_registry,
                level_registry,
                registry,
                rent,
                honest_v1_profile,
                pinned_registry,
                registry_moved,
                core_authority,
                consent,
                funder,
            },
        )
    }
}

/// Start one world's bank at the slot its single deployment generation is
/// visible and rooted at.
///
/// `ProgramTest` always starts at slot 1, where a ProgramData reporting 531
/// makes every program in the fixture invisible and the runtime reports it as a
/// program-cache replacement rather than as anything about the deployment.
async fn start(test: ProgramTest) -> ProgramTestContext {
    let mut context = test.start_with_context().await;
    context
        .warp_to_slot(BANK_SLOT)
        .expect("warp the bank one slot past the deployment generation");
    context
}

fn observation() -> Observation {
    Observation {
        slot: BANK_SLOT,
        unix_timestamp: 1_800_000_000,
        finality: Finality::Finalized,
    }
}

/// Read one account off the running bank as a finalized observation.
///
/// A vacant address is a real observation too — conjunct 6 reads exactly one —
/// so an absent account becomes the System-owned zero-lamport empty account the
/// runtime would present.
async fn observe(context: &mut ProgramTestContext, key: Pubkey) -> ObservedAccount {
    let account = account_at(context, key).await;
    ObservedAccount {
        observation: observation(),
        key,
        owner: account.owner,
        lamports: account.lamports,
        executable: account.executable,
        data: account.data,
    }
}

async fn account_at(context: &mut ProgramTestContext, key: Pubkey) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("banks client")
        .unwrap_or(Account {
            lamports: 0,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        })
}

async fn predecessor_record(
    context: &mut ProgramTestContext,
    record: Record,
) -> PredecessorRecordObservationV1 {
    PredecessorRecordObservationV1 {
        raw: observe(context, record.raw).await,
        staging: observe(context, record.staging).await,
    }
}

/// The shipped builder's inputs, observed off this world's running bank.
///
/// The caller states which predecessor records it is holding and the builder
/// checks that belief against the bytes rather than trusting it, so the Rent
/// binding — unmoved in every world here — presents none.
async fn succession_state(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> CoreInfrastructureSuccessionStateV1 {
    let pinned = fixture.pinned_registry;
    let registry_moved = fixture.registry_moved;
    CoreInfrastructureSuccessionStateV1 {
        payer: observe(context, fixture.funder.pubkey()).await,
        profile: observe(context, fixture.profile_v2).await,
        predecessor_profile: observe(context, fixture.profile_v1).await,
        core_programdata: observe(context, programdata_address(CORE_PROGRAM_ID)).await,
        upgrade_authority: observe(context, fixture.core_authority.pubkey()).await,
        registry_artifact_raw: observe(context, fixture.registry.raw).await,
        registry_artifact_staging: observe(context, fixture.registry.staging).await,
        registry_program: observe(context, REGISTRY_PROGRAM_ID).await,
        registry_programdata: observe(context, programdata_address(REGISTRY_PROGRAM_ID)).await,
        rent_artifact_raw: observe(context, fixture.rent.raw).await,
        rent_artifact_staging: observe(context, fixture.rent.staging).await,
        rent_program: observe(context, RENT_PROGRAM_ID).await,
        rent_programdata: observe(context, programdata_address(RENT_PROGRAM_ID)).await,
        predecessor_registry_record: match registry_moved {
            true => Some(predecessor_record(context, pinned).await),
            false => None,
        },
        predecessor_rent_record: None,
        rent_sysvar: observe(context, sysvar::rent::ID).await,
        system_program: observe(context, system_program::ID).await,
    }
}

/// The honest ceremony, composed by the builder, submittable into any world.
///
/// The builder refuses every doctored world locally — which is the point of it,
/// and which each of those tests asserts first — so a hostile has to go around
/// it to reach the compiled program at all. Going around it here means handing
/// it the cohort-9 predecessor evidence: none of the frame's twenty-one
/// ADDRESSES depends on the V1 profile's bytes, so the frame it composes is the
/// honest frame in every world, and the doctored bytes at the V1 PDA are what
/// the program reads when that frame is submitted.
async fn honest_frame_against(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> CoreInfrastructureSuccessionReportV1 {
    let mut state = succession_state(context, fixture).await;
    state.predecessor_profile.data = fixture.honest_v1_profile.to_bytes().to_vec();
    state.predecessor_registry_record =
        Some(predecessor_record(context, fixture.predecessor_registry).await);
    build_core_infrastructure_succession_v1(CORE_PROGRAM_ID, &state)
        .expect("the builder must admit the honest succession")
}

/// Clone the honest instruction and change exactly one thing about its frame.
fn perturbed(honest: &Instruction, edit: impl FnOnce(&mut Vec<AccountMeta>)) -> Instruction {
    let mut instruction = honest.clone();
    edit(&mut instruction.accounts);
    instruction
}

fn meta(accounts: &mut [AccountMeta], index: usize) -> &mut AccountMeta {
    accounts.get_mut(index).expect("frame slot")
}

/// Submit one ceremony behind an explicit compute-unit limit.
///
/// `limit` is a real parameter rather than a constant because a REPLAY of the
/// honest instruction is otherwise byte-identical to it: same message, same
/// signature, and the bank would reject the duplicate signature instead of
/// running conjunct 6. Varying the limit by one unit makes the replay a
/// distinct transaction that actually reaches the program.
async fn submit(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    signers: &[&Keypair],
    limit: u32,
) -> Result<(), TransactionError> {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let fee_payer = context.payer.insecure_clone();
    let mut all: Vec<&Keypair> = vec![&fee_payer];
    all.extend_from_slice(signers);
    let transaction = Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(limit),
            instruction,
        ],
        Some(&fee_payer.pubkey()),
        &all,
        blockhash,
    );
    context
        .banks_client
        .process_transaction(transaction)
        .await
        .map_err(|error| match error {
            BanksClientError::TransactionError(error) => error,
            BanksClientError::SimulationError { err, .. } => err,
            other => panic!("unexpected banks error: {other:?}"),
        })
}

/// Submit the ordinary way: the full ceremony budget, all three signatures.
async fn submit_signed(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    fixture: &Fixture,
) -> Result<(), TransactionError> {
    let signers = [&fixture.funder, &fixture.core_authority, &fixture.consent];
    submit(context, instruction, &signers, PROTOCOL_COMPUTE_UNIT_LIMIT).await
}

/// Assert the compiled program refused by NAME, not by a number typed by hand.
#[track_caller]
fn refused(result: Result<(), TransactionError>, expected: CoreSbfError) {
    let error = result.expect_err("this frame must be refused");
    assert_eq!(
        error,
        TransactionError::InstructionError(
            CEREMONY_INSTRUCTION_INDEX,
            InstructionError::Custom(expected as u32),
        ),
        "expected {expected:?}"
    );
}

/// Assert the V2 domain is still the vacancy the ceremony has yet to spend.
async fn assert_v2_still_vacant(context: &mut ProgramTestContext, fixture: &Fixture) {
    let account = account_at(context, fixture.profile_v2).await;
    assert_eq!(
        account.owner,
        system_program::ID,
        "a refused ceremony must leave the V2 domain System-owned"
    );
    assert!(
        account.data.is_empty(),
        "a refused ceremony must leave the V2 domain dataless"
    );
}

/// The whole positive campaign plus every hostile a frame perturbation reaches.
///
/// One bank, not eight, and deliberately: conjunct 6 refuses on the V2 account's
/// existence before anything else a hostile could be testing is reached, so
/// every hostile below has to run against a domain that is still vacant. One
/// bank makes that ordering a stated property — hostiles first on an untouched
/// address, then the honest ceremony, then the replay that must find the domain
/// spent.
#[tokio::test]
async fn the_compiled_core_admits_exactly_the_infrastructure_succession() {
    let (test, fixture) = Fixture::new(WorldV1::RegistryMoved);
    let mut context = start(test).await;

    // ---- what the shipped builder composes, before anything is sent ----
    let state = succession_state(&mut context, &fixture).await;
    assert_eq!(
        state.predecessor_profile.data,
        fixture.honest_v1_profile.to_bytes().to_vec(),
        "the cohort-9 world plants exactly the profile the builder is handed"
    );
    let report = build_core_infrastructure_succession_v1(CORE_PROGRAM_ID, &state)
        .expect("the builder must admit the honest succession");
    let honest = report.instruction.clone();

    assert_eq!(honest.program_id, CORE_PROGRAM_ID);
    assert_eq!(
        honest.accounts.len(),
        INITIALIZE_INFRASTRUCTURE_ACCOUNT_COUNT_V2
    );
    assert_eq!(report.profile, fixture.profile_v2);
    // The moved binding asks the predecessor's deployer to consent; the unmoved
    // one stands the System program and must not sign. That asymmetry is the
    // whole of conjunct 5 in one frame.
    let [registry_consent, rent_consent] = report.consent;
    assert_eq!(registry_consent.binding, InfrastructureBindingV1::Registry);
    assert!(registry_consent.moved && registry_consent.must_sign);
    assert_eq!(registry_consent.slot, fixture.consent.pubkey());
    assert_eq!(rent_consent.binding, InfrastructureBindingV1::Rent);
    assert!(!rent_consent.moved && !rent_consent.must_sign);
    assert_eq!(rent_consent.slot, system_program::ID);
    assert_eq!(
        report.required_signers,
        vec![
            fixture.funder.pubkey(),
            fixture.core_authority.pubkey(),
            fixture.consent.pubkey(),
        ],
        "three distinct keys: the funder, Core's Loader authority, the consenting deployer"
    );
    // The bytes the ceremony would persist, walkable back to the V1 profile it
    // succeeded.
    assert_eq!(report.record.registry(), fixture.registry.binding);
    assert_eq!(report.record.rent(), fixture.rent.binding);
    assert_eq!(
        report.record.predecessor_registry_artifact(),
        fixture.predecessor_registry.binding.artifact_release()
    );
    assert_eq!(
        report.record.predecessor_rent_artifact(),
        fixture.rent.binding.artifact_release(),
        "an unmoved binding carries the same artifact id on both sides"
    );

    let predecessor_before = account_at(&mut context, fixture.profile_v1).await;

    // ---- one hostile per reachable conjunct, on a still-vacant domain ----

    // An unsigned Core upgrade authority refuses as `AccountFrame`, not as
    // conjunct 1. `InitializeInfrastructureV2Accounts::parse` requires
    // `upgrade_authority.is_signer` before `process_initialize_v2` runs at all,
    // so `authenticate_current_core_upgrade_authority`'s own `!is_signer` clause
    // is UNREACHABLE from this frame. The layering is correct and the refusal is
    // the frame's own, but a campaign asserting `Infrastructure` here would be
    // asserting a code this route cannot produce.
    //
    // The edit is only a real coordinate because Core's authority is not also
    // standing in a consent slot: one key repeated in one message carries the
    // union of its privileges, so with the two aliased this would change nothing
    // and the frame would LAND what it meant to refuse.
    refused(
        submit(
            &mut context,
            perturbed(&honest, |accounts| {
                meta(accounts, CORE_UPGRADE_AUTHORITY_ACCOUNT).is_signer = false;
            }),
            &[&fixture.funder, &fixture.consent],
            PROTOCOL_COMPUTE_UNIT_LIMIT,
        )
        .await,
        CoreSbfError::AccountFrame,
    );
    assert_v2_still_vacant(&mut context, &fixture).await;

    // Conjunct 1 proper: a key that signs, but not the one Core's live
    // ProgramData binds. The party that could already replace the reader itself
    // is the only party that may re-select what it reads.
    let intruder = intruder();
    refused(
        submit(
            &mut context,
            perturbed(&honest, |accounts| {
                meta(accounts, CORE_UPGRADE_AUTHORITY_ACCOUNT).pubkey = intruder.pubkey();
            }),
            &[&fixture.funder, &intruder, &fixture.consent],
            PROTOCOL_COMPUTE_UNIT_LIMIT,
        )
        .await,
        CoreSbfError::Infrastructure,
    );
    assert_v2_still_vacant(&mut context, &fixture).await;

    // Conjunct 2: succession without a predecessor is initialization's job.
    refused(
        submit_signed(
            &mut context,
            perturbed(&honest, |accounts| {
                meta(accounts, PREDECESSOR_PROFILE_ACCOUNT).pubkey = UNPLANTED_ADDRESS;
            }),
            &fixture,
        )
        .await,
        CoreSbfError::InfrastructurePredecessorAbsent,
    );
    assert_v2_still_vacant(&mut context, &fixture).await;

    // Conjunct 5, the moved arm: the predecessor's bound deployer is present
    // and does not sign.
    refused(
        submit(
            &mut context,
            perturbed(&honest, |accounts| {
                meta(accounts, REGISTRY_CONSENT_ACCOUNT_V2).is_signer = false;
            }),
            &[&fixture.funder, &fixture.core_authority],
            PROTOCOL_COMPUTE_UNIT_LIMIT,
        )
        .await,
        CoreSbfError::InfrastructureConsentMissing,
    );
    assert_v2_still_vacant(&mut context, &fixture).await;

    // Conjunct 5 again: a signature, from the wrong key. The consenting key is
    // read out of the predecessor record, so no other signer substitutes for it.
    refused(
        submit(
            &mut context,
            perturbed(&honest, |accounts| {
                meta(accounts, REGISTRY_CONSENT_ACCOUNT_V2).pubkey = intruder.pubkey();
            }),
            &[&fixture.funder, &fixture.core_authority, &intruder],
            PROTOCOL_COMPUTE_UNIT_LIMIT,
        )
        .await,
        CoreSbfError::InfrastructureConsentMissing,
    );
    assert_v2_still_vacant(&mut context, &fixture).await;

    // Conjunct 5, the unmoved arm: nothing is being consented to for Rent, so
    // nothing may stand where consent would go — not even a key that consented
    // for the other binding.
    refused(
        submit_signed(
            &mut context,
            perturbed(&honest, |accounts| {
                meta(accounts, RENT_CONSENT_ACCOUNT_V2).pubkey = fixture.consent.pubkey();
            }),
            &fixture,
        )
        .await,
        CoreSbfError::InfrastructureConsentMissing,
    );
    assert_v2_still_vacant(&mut context, &fixture).await;

    // ---- the ceremony itself ----
    submit_signed(&mut context, honest.clone(), &fixture)
        .await
        .expect("the honest succession must land");

    let landed = account_at(&mut context, fixture.profile_v2).await;
    assert_eq!(landed.owner, CORE_PROGRAM_ID);
    assert_eq!(landed.data.len(), PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2);
    assert!(!landed.executable);
    assert!(
        Rent::default().is_exempt(landed.lamports, PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2),
        "the created profile must be rent-exempt"
    );
    // Conjunct 7 from outside: not "it decodes to something sensible" but "it is
    // byte-for-byte the image the builder projected before the transaction was
    // signed".
    assert_eq!(
        landed.data,
        report.record.to_bytes().to_vec(),
        "the persisted image must equal the bytes the builder composed"
    );
    assert_eq!(
        ProtocolInfrastructureProfileV2::decode(&landed.data),
        Ok(report.record)
    );

    // §8.3's byte-identity bar: V1 is read as evidence and never written. It
    // stays on chain byte-identical forever, a sealed historical record still
    // content-walkable from V2's predecessor artifact ids.
    let predecessor_after = account_at(&mut context, fixture.profile_v1).await;
    assert_eq!(
        predecessor_after.data, predecessor_before.data,
        "the V1 profile must be byte-identical after the succession"
    );
    assert_eq!(predecessor_after.owner, predecessor_before.owner);
    assert_eq!(predecessor_after.lamports, predecessor_before.lamports);
    assert_eq!(
        ProtocolInfrastructureProfileV1::decode(&predecessor_after.data),
        Ok(fixture.honest_v1_profile)
    );

    // Conjunct 6, the no-fork: one succession per domain, ever.
    refused(
        submit(
            &mut context,
            honest,
            &[&fixture.funder, &fixture.core_authority, &fixture.consent],
            PROTOCOL_COMPUTE_UNIT_LIMIT - 1,
        )
        .await,
        CoreSbfError::InfrastructureAlreadySucceeded,
    );
    let after_replay = account_at(&mut context, fixture.profile_v2).await;
    assert_eq!(
        after_replay.data, landed.data,
        "a refused replay must leave the succession profile untouched"
    );
    let predecessor_finally = account_at(&mut context, fixture.profile_v1).await;
    assert_eq!(
        predecessor_finally.data, predecessor_before.data,
        "the V1 profile must be byte-identical after everything above"
    );
}

/// Conjunct 3: bytes may move across a succession, identity never.
#[tokio::test]
async fn a_succession_that_relocates_the_registry_program_refuses_by_name() {
    let (test, fixture) = Fixture::new(WorldV1::RegistryRelocated);
    let mut context = start(test).await;

    let state = succession_state(&mut context, &fixture).await;
    assert_eq!(
        build_core_infrastructure_succession_v1(CORE_PROGRAM_ID, &state).err(),
        Some(SuccessionBuilderError::IdentityMoved),
        "the builder refuses this world locally, so the hostile has to go around it"
    );

    let honest = honest_frame_against(&mut context, &fixture)
        .await
        .instruction;
    refused(
        submit_signed(&mut context, honest, &fixture).await,
        CoreSbfError::InfrastructureIdentityMoved,
    );
    assert_v2_still_vacant(&mut context, &fixture).await;
}

/// Conjunct 4's forward arm: under Loader V3 a deployment slot only moves
/// forward, so a predecessor record binding a LATER slot is an upgrade claimed
/// to have run backwards.
#[tokio::test]
async fn a_succession_whose_predecessor_deployment_is_later_refuses_by_name() {
    let (test, fixture) = Fixture::new(WorldV1::PredecessorAhead);
    let mut context = start(test).await;

    let state = succession_state(&mut context, &fixture).await;
    assert_eq!(
        build_core_infrastructure_succession_v1(CORE_PROGRAM_ID, &state).err(),
        Some(SuccessionBuilderError::NotForward),
        "the builder refuses this world locally, so the hostile has to go around it"
    );

    // The predecessor record is content-pinned to the id THIS world's V1 profile
    // named, so reaching conjunct 4 means presenting that record — its raw
    // account and its cursor, which are one object and move together. Present
    // any other record and the digest pin refuses first, under a different name.
    let honest = honest_frame_against(&mut context, &fixture)
        .await
        .instruction;
    let hostile = perturbed(&honest, |accounts| {
        meta(accounts, PREDECESSOR_REGISTRY_RAW_ACCOUNT).pubkey =
            fixture.non_advancing_registry.raw;
        meta(accounts, PREDECESSOR_REGISTRY_STAGING_ACCOUNT).pubkey =
            fixture.non_advancing_registry.staging;
    });
    refused(
        submit_signed(&mut context, hostile, &fixture).await,
        CoreSbfError::InfrastructureNotForward,
    );
    assert_v2_still_vacant(&mut context, &fixture).await;
}

/// Conjunct 4's boundary: forward means STRICTLY forward.
///
/// The sharp case, and the one the mutation floor had to ask for. A predecessor
/// record binding a LATER slot is refused by any comparison that rejects going
/// backwards, so the sibling test above cannot tell `<=` from `<`. Only an
/// EQUAL slot separates them, and relaxing the route to `<` survived the whole
/// campaign until this existed. Standing still is not a succession: the record
/// differs, so the ids differ and the binding reads as moved, but the bytes
/// behind it are the ones the predecessor already selected -- consent would be
/// asked and a vacancy spent for a re-selection of the same deployment.
#[tokio::test]
async fn a_succession_whose_predecessor_deployment_is_level_refuses_by_name() {
    let (test, fixture) = Fixture::new(WorldV1::PredecessorLevel);
    let mut context = start(test).await;

    let state = succession_state(&mut context, &fixture).await;
    assert_eq!(
        build_core_infrastructure_succession_v1(CORE_PROGRAM_ID, &state).err(),
        Some(SuccessionBuilderError::NotForward),
        "the builder refuses this world locally, so the hostile has to go around it"
    );

    let honest = honest_frame_against(&mut context, &fixture)
        .await
        .instruction;
    let hostile = perturbed(&honest, |accounts| {
        meta(accounts, PREDECESSOR_REGISTRY_RAW_ACCOUNT).pubkey = fixture.level_registry.raw;
        meta(accounts, PREDECESSOR_REGISTRY_STAGING_ACCOUNT).pubkey =
            fixture.level_registry.staging;
    });
    refused(
        submit_signed(&mut context, hostile, &fixture).await,
        CoreSbfError::InfrastructureNotForward,
    );
    assert_v2_still_vacant(&mut context, &fixture).await;
}

/// Conjunct 4's degenerate arm: a succession in which nothing moved selects
/// nothing new and would spend the one vacancy this domain will ever have.
#[tokio::test]
async fn a_succession_that_moves_nothing_refuses_before_it_spends_the_vacancy() {
    let (test, fixture) = Fixture::new(WorldV1::NothingMoved);
    let mut context = start(test).await;
    assert!(!fixture.registry_moved);

    let state = succession_state(&mut context, &fixture).await;
    assert_eq!(
        build_core_infrastructure_succession_v1(CORE_PROGRAM_ID, &state).err(),
        Some(SuccessionBuilderError::NothingMoved),
        "the builder refuses this world locally, so the hostile has to go around it"
    );

    // The frame still carries the cohort-9 predecessor evidence and a consenting
    // signature; the degenerate arm fires before either is read, which is what
    // makes this hostile about conjunct 4 rather than about conjunct 5.
    let honest = honest_frame_against(&mut context, &fixture)
        .await
        .instruction;
    refused(
        submit_signed(&mut context, honest, &fixture).await,
        CoreSbfError::InfrastructureNotForward,
    );
    assert_v2_still_vacant(&mut context, &fixture).await;
}
