//! The real-SVM campaign for `DeclareSuccessor`, driving the compiled Registry.
//!
//! The route's unit suite states its own gap twice, in the source:
//!
//! > The creation is a System CPI that no unit-test runtime can serve, so the
//! > admitted path is exercised here and the created account is commit 10's
//! > real-SVM campaign.
//! > -- `src/tests.rs`, on `LineageFixture::compose`
//!
//! So `create_lineage_record` -- the `invoke_signed`, the write-back, and
//! conjunct 8's read-back belt -- had never executed anywhere. This is that
//! campaign. Every transaction below goes through the compiled
//! `dclutch_registry_sbf.so`, not through `process_instruction` in-process.
//!
//! The honest declaration is built by the SHIPPED host builder
//! (`dclutch_operator::registry::declare_successor_v1`), because the thing
//! actually at risk is not whether the program is correct -- twelve unit tests
//! already say so -- but whether the only tool that can call it produces a
//! frame the program accepts. A campaign that hand-assembled its own frame
//! would prove the program right and leave the builder untested.
//!
//! The hostiles do hand-assemble, and must: the builder refuses seven of them
//! locally, which is the point of it. To reach the program at all a hostile has
//! to go around the builder, so each one starts from the honest frame the
//! builder produced and perturbs exactly one coordinate.
//!
//! Run: `programs/dclutch-registry-sbf/run-lineage-program-test.sh`

use std::{collections::BTreeMap, env, fs, path::PathBuf};

use dclutch_core_contract::ContentId;
use dclutch_operator::{
    Finality, Observation, ObservedAccount,
    registry::declare_successor_v1::{
        RegistryDeclareSuccessorState, build_registry_declare_successor_v1,
        declare_successor_frame_addresses_v1,
    },
};
use dclutch_registry::activation_auth_v1::{
    activation_cache_address_v1, release_lineage_address_and_bump_v1,
};
use dclutch_registry::release_set::{
    ArtifactReleaseIdV1, EXECUTION_ROLE_ORDER_V1, ExecutionReleaseSetV1, ExecutionRoleBindingV1,
    ExecutionRoleV1, ProgramIdentityV1,
};
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, LineageAt, RELEASE_LINEAGE_BYTES_V1,
    ReleaseLineageV1, activate_execution_role_into_v1, initialize_activation_cache_v1,
    put_activation_cache_bump_v1, walk_lineage_to, walk_lineage_to_head,
};
use dclutch_registry_sbf::RegistryError;
use solana_account::Account;
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::{
    hash::hash,
    instruction::{AccountMeta, Instruction, InstructionError},
    pubkey::Pubkey,
    rent::Rent,
    signature::{Keypair, Signer},
    sysvar,
    transaction::TransactionError,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};
use solana_transaction::Transaction;

/// Where the compiled Registry is deployed for this campaign.
///
/// Any address works -- both caches are derived UNDER it, so the fixture is
/// self-consistent whatever this is. It is not the devnet Registry, because
/// nothing here should read as a claim about devnet.
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x9e; 32]);

/// The five roles' shared upgrade authority.
///
/// One key for all five is not a simplification: it is what devnet actually
/// has. All five roles in all eight recovered release sets bind
/// `4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP`, the retained Loader
/// deployer, so every hop needs exactly one consenting signature and no other.
/// A fixture with five distinct authorities would exercise a coalition the
/// cluster does not have and would hide the deduplication the cut relies on.
fn authority_keypair() -> Keypair {
    Keypair::new_from_array([0x44; 32])
}

/// One role's deployment inside a release set.
#[derive(Clone, Copy)]
struct RoleSpec {
    program: u8,
    elf: u8,
    slot: u64,
    authority: Option<Pubkey>,
}

/// Cohort-7's shape.
fn predecessor_specs(authority: Pubkey) -> [RoleSpec; 5] {
    [
        RoleSpec {
            program: 0x11,
            elf: 0x71,
            slot: 490_697_000,
            authority: Some(authority),
        },
        RoleSpec {
            program: 0x12,
            elf: 0x72,
            slot: 490_697_100,
            authority: Some(authority),
        },
        RoleSpec {
            program: 0x13,
            elf: 0x73,
            slot: 490_697_200,
            authority: Some(authority),
        },
        RoleSpec {
            program: 0x14,
            elf: 0x74,
            slot: 490_693_331,
            authority: Some(authority),
        },
        RoleSpec {
            program: 0x15,
            elf: 0x75,
            slot: 490_697_400,
            authority: Some(authority),
        },
    ]
}

/// Cohort-8's shape: four roles moved, resolution byte-identical.
///
/// The unmoved role is real, not decorative. Cohort-8's resolution deployment
/// slot 490693331 IS cohort-7's -- that role did not move across the hop -- and
/// it is the coordinate that makes conjunct 6 asymmetric.
fn successor_specs(authority: Pubkey) -> [RoleSpec; 5] {
    [
        RoleSpec {
            program: 0x11,
            elf: 0x81,
            slot: 490_849_793,
            authority: Some(authority),
        },
        RoleSpec {
            program: 0x12,
            elf: 0x82,
            slot: 490_826_560,
            authority: Some(authority),
        },
        RoleSpec {
            program: 0x13,
            elf: 0x83,
            slot: 490_830_840,
            authority: Some(authority),
        },
        RoleSpec {
            program: 0x14,
            elf: 0x74,
            slot: 490_693_331,
            authority: Some(authority),
        },
        RoleSpec {
            program: 0x15,
            elf: 0x85,
            slot: 490_814_947,
            authority: Some(authority),
        },
    ]
}

/// The same hop with resolution moved too: six of the seven recovered hops.
///
/// Hops 1-to-2 through 6-to-7 moved all five roles, so this is the ordinary
/// shape and not a simplification of the devnet one. It is the shape the main
/// landing campaign uses; the four-moved shape -- the seventh hop, cohort-7 to
/// cohort-8 -- gets its own bank below, because an unmoved role is the only
/// coordinate that makes conjunct 6 asymmetric.
fn all_moved_successor_specs(authority: Pubkey) -> [RoleSpec; 5] {
    let mut specs = successor_specs(authority);
    if let Some(spec) = specs.get_mut(ExecutionRoleV1::Resolution.role_index()) {
        spec.elf = 0x84;
        spec.slot = 490_845_000;
    }
    specs
}

/// Whether each role's artifact release id differs across the hop.
fn moved_mask(before: [RoleSpec; 5], after: [RoleSpec; 5]) -> [bool; 5] {
    let mut mask = [false; 5];
    for (index, slot) in mask.iter_mut().enumerate() {
        let before = artifact_id(release_for(*before.get(index).expect("role")));
        let after = artifact_id(release_for(*after.get(index).expect("role")));
        *slot = before != after;
    }
    mask
}

fn release_for(spec: RoleSpec) -> ArtifactReleaseV1 {
    let program = Pubkey::new_from_array([spec.program; 32]);
    let programdata =
        Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0;
    ArtifactReleaseV1::new(
        ProgramIdentityV1::new(program.to_bytes()).expect("program identity"),
        ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader identity"),
        programdata.to_bytes(),
        ContentId::new([spec.program | 0x01; 32]).expect("semantic release"),
        [spec.elf; 32],
        spec.slot,
        match spec.authority {
            Some(_) => ArtifactUpgradePolicyV1::ExactAuthority,
            None => ArtifactUpgradePolicyV1::Immutable,
        },
        spec.authority.map(|key| key.to_bytes()),
    )
    .expect("artifact release")
}

fn artifact_id(release: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes()).expect("artifact release id")
}

/// The activation input the Registry would have written for this release.
///
/// The declaration route reads only the two cache accounts and never observes a
/// deployment, so composing the cache from the release itself is the same input
/// the route sees on chain rather than a shortcut past a check it makes.
fn activation_input(release: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
    ArtifactActivationInputV1::new(
        artifact_id(release),
        release,
        DeploymentObservationV1::new(
            release.program().to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            true,
            release.programdata(),
            bpf_loader_upgradeable::ID.to_bytes(),
            false,
            release.programdata(),
            bpf_loader_upgradeable::ID.to_bytes(),
            release.deployment_slot(),
            release.elf_digest(),
            release.upgrade_authority(),
        )
        .expect("deployment observation"),
    )
}

/// One complete activation cache, as the Registry writes them.
struct CacheFixture {
    release_set_id: ContentId,
    address: Pubkey,
    bytes: Vec<u8>,
}

fn build_cache(specs: [RoleSpec; 5]) -> CacheFixture {
    let releases: Vec<ArtifactReleaseV1> = specs.iter().copied().map(release_for).collect();
    let bindings: Vec<ExecutionRoleBindingV1> = releases
        .iter()
        .map(|release| ExecutionRoleBindingV1::new(release.program(), artifact_id(*release)))
        .collect();
    let [core, claims, trading, resolution, custody]: [ExecutionRoleBindingV1; 5] =
        bindings.try_into().ok().expect("five bindings");
    let release_set = ExecutionReleaseSetV1::new(core, claims, trading, resolution, custody)
        .expect("execution release set");
    let release_set_id =
        ContentId::new(hash(&release_set.to_bytes()).to_bytes()).expect("release set id");

    let mut bytes = vec![0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut bytes, release_set_id).expect("initialize cache");
    for role in EXECUTION_ROLE_ORDER_V1 {
        let release = *releases.get(role.role_index()).expect("role release");
        activate_execution_role_into_v1(
            &mut bytes,
            release_set_id,
            &release_set,
            role,
            &activation_input(release),
        )
        .expect("activate role");
    }
    let (address, bump) = Pubkey::find_program_address(
        &[
            dclutch_registry::ACTIVATION_PDA_DOMAIN_V1,
            release_set_id.as_bytes(),
        ],
        &REGISTRY_PROGRAM_ID,
    );
    // The real Registry records this at activation and every reader reproduces
    // the address from it. A fixture leaving it zero would stage an account no
    // deployment produces.
    put_activation_cache_bump_v1(&mut bytes, bump).expect("cache bump");
    CacheFixture {
        release_set_id,
        address,
        bytes,
    }
}

fn registry_account(data: Vec<u8>) -> Account {
    Account {
        lamports: Rent::default().minimum_balance(data.len()),
        data,
        owner: REGISTRY_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    }
}

struct Fixture {
    predecessor: CacheFixture,
    successor: CacheFixture,
    authority: Keypair,
    /// Funds the record, and is NOT the transaction fee payer.
    ///
    /// Conjunct 1 requires the frame's payer to be a writable signer, and that
    /// clause is unreachable while the frame's payer IS the fee payer: the fee
    /// payer occupies message index 0 and the compiler gives it both privileges
    /// no matter what the instruction's meta asked for, so a hostile that
    /// cleared the writable bit would be silently discarded and would LAND the
    /// declaration it was meant to refuse. Separating the two makes the clause
    /// a real coordinate the test can vary.
    funder: Keypair,
    lineage: Pubkey,
}

impl Fixture {
    fn new() -> (ProgramTest, Self) {
        let directory = PathBuf::from(
            env::var("SBF_OUT_DIR")
                .expect("SBF_OUT_DIR is required: build dclutch_registry_sbf.so first"),
        );
        let elf = fs::read(directory.join("dclutch_registry_sbf.so"))
            .expect("compiled Registry ELF in SBF_OUT_DIR");
        assert_eq!(
            elf.get(..4),
            Some(&[0x7f, b'E', b'L', b'F'][..]),
            "the Registry artifact must be a real ELF"
        );
        eprintln!(
            "Registry ELF: {} bytes, sha256 {:?}",
            elf.len(),
            hash(&elf).to_bytes()
        );

        let authority = authority_keypair();
        let predecessor = build_cache(predecessor_specs(authority.pubkey()));
        let successor = build_cache(all_moved_successor_specs(authority.pubkey()));
        assert_ne!(
            predecessor.release_set_id, successor.release_set_id,
            "the two endpoints must be different sets"
        );

        let mut test = ProgramTest::default();
        test.prefer_bpf(true);
        test.set_compute_max_units(1_400_000);
        test.add_upgradeable_program_to_genesis("dclutch_registry_sbf", &REGISTRY_PROGRAM_ID);
        test.add_account(
            predecessor.address,
            registry_account(predecessor.bytes.clone()),
        );
        test.add_account(successor.address, registry_account(successor.bytes.clone()));
        let funder = Keypair::new_from_array([0x2f; 32]);
        test.add_account(
            funder.pubkey(),
            Account {
                lamports: 10_000_000_000,
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            },
        );

        let lineage = release_lineage_address_and_bump_v1(
            &REGISTRY_PROGRAM_ID,
            predecessor.release_set_id.as_bytes(),
        )
        .0;
        (
            test,
            Self {
                predecessor,
                successor,
                authority,
                funder,
                lineage,
            },
        )
    }
}

fn observation() -> Observation {
    Observation {
        slot: 1,
        unix_timestamp: 1_800_000_000,
        finality: Finality::Finalized,
    }
}

/// Read one account off the running bank as a finalized observation.
///
/// A vacant address is a real observation too -- it is what conjunct 7 reads --
/// so an absent account becomes the System-owned zero-lamport empty account the
/// runtime would present, not an error.
async fn observe(context: &mut ProgramTestContext, key: Pubkey) -> ObservedAccount {
    let account = context
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
        });
    ObservedAccount {
        observation: observation(),
        key,
        owner: account.owner,
        lamports: account.lamports,
        executable: account.executable,
        data: account.data,
    }
}

async fn declare_state(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> RegistryDeclareSuccessorState {
    let payer = fixture.funder.pubkey();
    RegistryDeclareSuccessorState {
        payer: observe(context, payer).await,
        lineage: observe(context, fixture.lineage).await,
        predecessor_cache: observe(context, fixture.predecessor.address).await,
        successor_cache: observe(context, fixture.successor.address).await,
        system_program: observe(context, system_program::ID).await,
        rent_sysvar: observe(context, sysvar::rent::ID).await,
    }
}

async fn submit(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    signers: &[&Keypair],
) -> Result<(), TransactionError> {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let payer = context.payer.insecure_clone();
    let mut all: Vec<&Keypair> = vec![&payer];
    all.extend_from_slice(signers);
    let transaction =
        Transaction::new_signed_with_payer(&[instruction], Some(&payer.pubkey()), &all, blockhash);
    context
        .banks_client
        .process_transaction(transaction)
        .await
        .map_err(|error| match error {
            solana_program_test::BanksClientError::TransactionError(error) => error,
            solana_program_test::BanksClientError::SimulationError { err, .. } => err,
            other => panic!("unexpected banks error: {other:?}"),
        })
}

/// Assert the compiled program refused by NAME, not by a number typed by hand.
#[track_caller]
fn refused(result: Result<(), TransactionError>, expected: RegistryError) {
    let error = result.expect_err("this frame must be refused");
    assert_eq!(
        error,
        TransactionError::InstructionError(0, InstructionError::Custom(expected as u32)),
        "expected {expected:?}"
    );
}

/// Read the lineage record at one predecessor's derived address.
async fn lineage_at(context: &mut ProgramTestContext, predecessor: ContentId) -> LineageAt {
    let address =
        release_lineage_address_and_bump_v1(&REGISTRY_PROGRAM_ID, predecessor.as_bytes()).0;
    match context
        .banks_client
        .get_account(address)
        .await
        .expect("banks client")
    {
        None => LineageAt::Undeclared,
        Some(account) if account.lamports == 0 && account.data.is_empty() => LineageAt::Undeclared,
        Some(account) => match ReleaseLineageV1::decode(&account.data) {
            Ok(record) => LineageAt::Declared(record),
            Err(error) => LineageAt::Undecodable(error),
        },
    }
}

/// The whole campaign in one bank.
///
/// One `#[tokio::test]`, not fifteen, and deliberately: every hostile below has
/// to run against a bank where the lineage account is still PRISTINE, because
/// conjunct 7 refuses on the account's existence before anything else it could
/// have been testing is reached. Fifteen tests would be fifteen banks and
/// fifteen ELF loads for the same fact; one bank makes the ordering a stated
/// property -- hostiles first, on an untouched address, then the honest
/// declaration, then the replay that must find the address taken.
#[tokio::test]
async fn the_compiled_registry_declares_a_successor_and_the_walk_follows_the_hop() {
    let (test, fixture) = Fixture::new();
    let mut context = test.start_with_context().await;

    // ---- the frame the shipped builder produces, before anything is sent ----
    let state = declare_state(&mut context, &fixture).await;
    let report = build_registry_declare_successor_v1(REGISTRY_PROGRAM_ID, &state)
        .expect("the builder must admit the honest hop");
    assert_eq!(report.predecessor, fixture.predecessor.release_set_id);
    assert_eq!(report.successor, fixture.successor.release_set_id);
    assert_eq!(report.lineage, fixture.lineage);
    assert_eq!(
        report.required_signers,
        vec![fixture.funder.pubkey(), fixture.authority.pubkey()],
        "five roles sharing one deployer need two signatures, not six"
    );
    let honest = report.instruction.clone();

    // ---- conjunct 1: the frame, exactly as tabled ----
    refused(
        submit(
            &mut context,
            perturbed(&honest, |accounts| {
                accounts.get_mut(0).expect("payer").is_writable = false;
            }),
            &[&fixture.funder, &fixture.authority],
        )
        .await,
        RegistryError::AccountFrame,
    );
    refused(
        submit(
            &mut context,
            perturbed(&honest, |accounts| {
                accounts.get_mut(2).expect("predecessor cache").is_writable = true;
            }),
            &[&fixture.funder, &fixture.authority],
        )
        .await,
        RegistryError::AccountFrame,
    );
    refused(
        submit(
            &mut context,
            perturbed(&honest, |accounts| {
                accounts.truncate(10);
            }),
            &[&fixture.funder, &fixture.authority],
        )
        .await,
        RegistryError::AccountFrame,
    );

    refused(
        submit(
            &mut context,
            perturbed(&honest, |accounts| {
                accounts.get_mut(4).expect("core consent").is_writable = true;
            }),
            &[&fixture.funder, &fixture.authority],
        )
        .await,
        RegistryError::AccountFrame,
    );
    // A consent slot exempts exactly one executable account, the System Program
    // an unmoved role's slot must hold. Any OTHER program standing there is
    // refused at the frame, which is what the exemption must not cost.
    refused(
        submit(
            &mut context,
            perturbed(&honest, |accounts| {
                let meta = accounts.get_mut(4).expect("core consent");
                meta.pubkey = REGISTRY_PROGRAM_ID;
                meta.is_signer = false;
            }),
            &[&fixture.funder, &fixture.authority],
        )
        .await,
        RegistryError::AccountFrame,
    );

    // ---- conjunct 2: a Registry-owned cache at its own derived address ----
    // The successor slot carries the PREDECESSOR's cache: a real, valid,
    // Registry-owned cache -- just not the one this address derives.
    refused(
        submit(
            &mut context,
            perturbed(&honest, |accounts| {
                accounts.get_mut(3).expect("successor cache").pubkey =
                    activation_cache_address_v1(&REGISTRY_PROGRAM_ID, &[0x5a; 32]);
            }),
            &[&fixture.funder, &fixture.authority],
        )
        .await,
        RegistryError::ActivationCache,
    );

    // ---- conjunct 3: a set is not its own successor ----
    refused(
        submit(
            &mut context,
            perturbed(&honest, |accounts| {
                accounts.get_mut(3).expect("successor cache").pubkey = fixture.predecessor.address;
            }),
            &[&fixture.funder, &fixture.authority],
        )
        .await,
        RegistryError::ReleaseLineageSelfSuccession,
    );

    // ---- conjunct 6: the consenting signature ----
    // The authority stands in all five moved slots and simply does not sign.
    refused(
        submit(
            &mut context,
            perturbed(&honest, |accounts| {
                for index in 4..9 {
                    accounts.get_mut(index).expect("consent slot").is_signer = false;
                }
            }),
            &[&fixture.funder],
        )
        .await,
        RegistryError::ReleaseLineageAuthorityMissing,
    );
    // "All five but one" is NOT expressible here, and the reason is worth
    // recording rather than working around. All five slots hold the same key,
    // and message compilation deduplicates an account into one entry carrying
    // the UNION of the privileges its references asked for -- so clearing the
    // signer bit on one slot while four others still set it compiles to an
    // account that signs, and the perturbation silently vanishes. The coalition
    // property needs roles that bind DIFFERENT authorities to be a real
    // coordinate at all, and it is tested where it is one:
    // `every_moved_role_needs_its_own_authority`.
    // A stranger signing in a moved role's slot is not consent: the key must be
    // the one the successor's cache binds, not merely a signature.
    let stranger = Keypair::new_from_array([0x5c; 32]);
    refused(
        submit(
            &mut context,
            perturbed(&honest, |accounts| {
                accounts.get_mut(4).expect("core consent").pubkey = stranger.pubkey();
            }),
            &[&fixture.funder, &fixture.authority, &stranger],
        )
        .await,
        RegistryError::ReleaseLineageAuthorityMissing,
    );

    // ---- conjunct 7: the record's address ----
    refused(
        submit(
            &mut context,
            perturbed(&honest, |accounts| {
                accounts.get_mut(1).expect("lineage").pubkey = release_lineage_address_and_bump_v1(
                    &REGISTRY_PROGRAM_ID,
                    fixture.successor.release_set_id.as_bytes(),
                )
                .0;
            }),
            &[&fixture.funder, &fixture.authority],
        )
        .await,
        RegistryError::AccountFrame,
    );

    // Nothing above created anything: the address the honest hop needs is still
    // pristine, which is what makes the ordering above sound.
    assert!(
        matches!(
            lineage_at(&mut context, fixture.predecessor.release_set_id).await,
            LineageAt::Undeclared
        ),
        "no refused frame may leave a record behind"
    );

    // ---- the honest declaration ----
    submit(
        &mut context,
        honest.clone(),
        &[&fixture.funder, &fixture.authority],
    )
    .await
    .expect("the compiled Registry must admit the builder's frame");

    // ---- what actually landed ----
    let landed = context
        .banks_client
        .get_account(fixture.lineage)
        .await
        .expect("banks client")
        .expect("the lineage record must exist");
    assert_eq!(landed.owner, REGISTRY_PROGRAM_ID);
    assert!(!landed.executable);
    assert_eq!(landed.data.len(), RELEASE_LINEAGE_BYTES_V1);
    assert_eq!(
        landed.lamports,
        Rent::default().minimum_balance(RELEASE_LINEAGE_BYTES_V1),
    );
    // Conjunct 8's belt, observed from outside: the bytes the program read back
    // and the bytes the host builder composed are the same 248 bytes.
    assert_eq!(
        landed.data,
        report.record.to_bytes(),
        "the landed record must be byte-equal to the one the builder projected"
    );
    let decoded = ReleaseLineageV1::decode(&landed.data).expect("the landed record must decode");
    assert_eq!(decoded, report.record);
    assert_eq!(decoded.predecessor(), fixture.predecessor.release_set_id);
    assert_eq!(decoded.successor(), fixture.successor.release_set_id);
    for role in EXECUTION_ROLE_ORDER_V1 {
        assert!(decoded.moved(role), "{role:?} moved across this hop");
        assert_eq!(
            decoded.consenting_authority(role),
            Some(fixture.authority.pubkey().to_bytes()),
        );
    }

    // ---- MIGRATE's walk authority follows the hop, end to end ----
    let mut chain = BTreeMap::new();
    for endpoint in [
        fixture.predecessor.release_set_id,
        fixture.successor.release_set_id,
    ] {
        chain.insert(
            endpoint.as_bytes().to_vec(),
            lineage_at(&mut context, endpoint).await,
        );
    }
    let lookup = |set: ContentId| {
        *chain
            .get(set.as_bytes().as_slice())
            .unwrap_or(&LineageAt::Undeclared)
    };
    let to_head = walk_lineage_to_head(fixture.predecessor.release_set_id, lookup)
        .expect("the chain has a head");
    assert_eq!(to_head.endpoint(), fixture.successor.release_set_id);
    assert_eq!(to_head.hops(), 1);
    assert!(!to_head.is_already_current());
    let to_destination = walk_lineage_to(
        fixture.predecessor.release_set_id,
        fixture.successor.release_set_id,
        lookup,
    )
    .expect("the destination is reachable");
    assert_eq!(to_destination.hops(), 1);
    // The head of the new world owes nothing, and says so as arrival.
    let at_head = walk_lineage_to_head(fixture.successor.release_set_id, lookup)
        .expect("the successor is its own head");
    assert_eq!(at_head.hops(), 0);
    assert!(at_head.is_already_current());

    // ---- conjunct 7 again, now that the address is taken: lineage never forks ----
    let state = declare_state(&mut context, &fixture).await;
    assert!(
        build_registry_declare_successor_v1(REGISTRY_PROGRAM_ID, &state).is_err(),
        "the builder must refuse a second declaration for the same predecessor"
    );
    refused(
        submit(&mut context, honest, &[&fixture.funder, &fixture.authority]).await,
        RegistryError::ReleaseLineageAlreadyDeclared,
    );

    eprintln!(
        "landed lineage record at {} ({} bytes)\n  predecessor {}\n  successor   {}\n  hex {}",
        fixture.lineage,
        landed.data.len(),
        hex(fixture.predecessor.release_set_id.as_bytes()),
        hex(fixture.successor.release_set_id.as_bytes()),
        hex(&landed.data),
    );
}

/// Conjuncts 4 and 5 need caches that disagree about a role, so they get their
/// own bank rather than a perturbed frame.
///
/// A hop whose role identity moved, and a hop whose moved role ran backwards in
/// slot, are properties of the two ACCOUNTS and not of the frame. There is no
/// way to express either by permuting the eleven metas, so a second fixture is
/// the honest way to reach them.
#[tokio::test]
async fn a_hop_may_move_a_roles_bytes_but_never_its_identity_and_never_backwards() {
    let authority = authority_keypair();

    // Conjunct 4: Core's program id differs across the hop.
    let mut moved_identity = all_moved_successor_specs(authority.pubkey());
    if let Some(spec) = moved_identity.first_mut() {
        spec.program = 0x21;
    }
    drive_refusal(
        predecessor_specs(authority.pubkey()),
        moved_identity,
        &authority,
        RegistryError::ReleaseLineageRoleIdentityMoved,
    )
    .await;

    // Conjunct 5: Core moved, and its deployment slot did not advance.
    let mut backwards = all_moved_successor_specs(authority.pubkey());
    if let Some(spec) = backwards.first_mut() {
        spec.slot = 1;
    }
    drive_refusal(
        predecessor_specs(authority.pubkey()),
        backwards,
        &authority,
        RegistryError::ReleaseLineageNotForward,
    )
    .await;

    // Conjunct 6's contradiction arm: an Immutable artifact binds nobody, so a
    // hop claiming it moved has no authority to ask.
    let mut immutable = all_moved_successor_specs(authority.pubkey());
    if let Some(spec) = immutable.first_mut() {
        spec.authority = None;
    }
    drive_refusal(
        predecessor_specs(authority.pubkey()),
        immutable,
        &authority,
        RegistryError::ReleaseLineageAuthorityMissing,
    )
    .await;
}

/// Stand up one bank for a hop the two caches themselves make illegal, hand the
/// program the frame it would have received, and require the named refusal.
async fn drive_refusal(
    predecessor_specs: [RoleSpec; 5],
    successor_specs: [RoleSpec; 5],
    authority: &Keypair,
    expected: RegistryError,
) {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    assert!(
        directory.join("dclutch_registry_sbf.so").exists(),
        "compiled Registry ELF in SBF_OUT_DIR"
    );
    let predecessor = build_cache(predecessor_specs);
    let successor = build_cache(successor_specs);
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    test.add_upgradeable_program_to_genesis("dclutch_registry_sbf", &REGISTRY_PROGRAM_ID);
    test.add_account(
        predecessor.address,
        registry_account(predecessor.bytes.clone()),
    );
    test.add_account(successor.address, registry_account(successor.bytes.clone()));
    let mut context = test.start_with_context().await;

    let addresses = declare_successor_frame_addresses_v1(
        REGISTRY_PROGRAM_ID,
        predecessor.release_set_id.as_bytes(),
        successor.release_set_id.as_bytes(),
    );
    // Hand-assembled on purpose: the builder refuses each of these locally, so
    // a frame that reaches the program has to be built around it. Every moved
    // role's slot carries the authority as a signer, which is the MOST
    // permissive frame the caller could present -- so the refusal that comes
    // back is the conjunct under test and not a missing signature.
    let mut accounts = vec![
        AccountMeta::new(context.payer.pubkey(), true),
        AccountMeta::new(addresses.lineage, false),
        AccountMeta::new_readonly(predecessor.address, false),
        AccountMeta::new_readonly(successor.address, false),
    ];
    let moved = moved_mask(predecessor_specs, successor_specs);
    for role in EXECUTION_ROLE_ORDER_V1 {
        accounts.push(if *moved.get(role.role_index()).expect("role") {
            AccountMeta::new_readonly(authority.pubkey(), true)
        } else {
            AccountMeta::new_readonly(system_program::ID, false)
        });
    }
    accounts.push(AccountMeta::new_readonly(system_program::ID, false));
    accounts.push(AccountMeta::new_readonly(sysvar::rent::ID, false));
    let instruction = Instruction {
        program_id: REGISTRY_PROGRAM_ID,
        accounts,
        data: dclutch_registry::svm::lineage_v1::DeclareSuccessorV1::to_bytes().to_vec(),
    };
    refused(
        submit(&mut context, instruction, &[authority]).await,
        expected,
    );
    assert!(
        matches!(
            lineage_at(&mut context, predecessor.release_set_id).await,
            LineageAt::Undeclared
        ),
        "a refused hop must leave nothing behind"
    );
}

/// A hop with an UNMOVED role frames, declares, and records the role as unmoved.
///
/// This hop was undeclarable by any caller until conjunct 1 grew one clause.
/// Conjunct 6 requires an unmoved role's slot to hold exactly
/// `system_program::ID` and not sign; conjunct 1 refused any consent slot whose
/// account is executable; and the System Program account IS executable -- which
/// this test still MEASURES rather than assuming, because the measurement is
/// what the whole clause rests on. The two were mutually unsatisfiable, so every
/// frame carrying an unmoved role was refused `AccountFrame` before either cache
/// was read. Six of the seven devnet hops moved all five roles and never met it;
/// the seventh is cohort-7 to cohort-8, whose resolution role did not move, and
/// that is precisely the hop cut gate 6 asks to be declared.
///
/// The clause exempts a consent slot holding exactly `system_program::ID` from
/// the executable refusal, and nothing else. What conjunct 1 protects -- that a
/// program, which holds no private key, cannot stand where a consent goes --
/// is proven still standing here in both directions that could cost it:
///
///   - a program that is NOT the System Program, in either a moved or an
///     unmoved role's slot, is still refused `AccountFrame` at the frame; and
///   - the System Program in a MOVED role's slot is refused by conjunct 6,
///     which wants a signature that account cannot produce, so a role cannot be
///     dressed up as unmoved to skip its authority.
///
/// The unit suite could not see the original defect. Its fixture built the
/// unmoved slot as `account(system_program::ID, .., native_loader::ID,
/// /* executable */ false)` -- an account with the System Program's address and
/// without its executable bit, which no runtime presents. That fixture now goes
/// through one `system_program_account()` that presents what runtimes present,
/// so the unit suite feels this clause too.
#[tokio::test]
async fn a_hop_with_an_unmoved_role_frames_and_declares_and_records_the_role_as_unmoved() {
    let authority = authority_keypair();
    let predecessor = build_cache(predecessor_specs(authority.pubkey()));
    // The real cohort-7 to cohort-8 shape: resolution byte-identical.
    let successor = build_cache(successor_specs(authority.pubkey()));
    assert_eq!(
        moved_mask(
            predecessor_specs(authority.pubkey()),
            successor_specs(authority.pubkey())
        ),
        [true, true, true, false, true],
        "the fixture must be the four-moved devnet shape"
    );

    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    assert!(directory.join("dclutch_registry_sbf.so").exists());
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    test.add_upgradeable_program_to_genesis("dclutch_registry_sbf", &REGISTRY_PROGRAM_ID);
    test.add_account(
        predecessor.address,
        registry_account(predecessor.bytes.clone()),
    );
    test.add_account(successor.address, registry_account(successor.bytes.clone()));
    // A program that is not the System Program, for the refusing direction. It
    // is staged rather than borrowed from genesis so the hostile depends on no
    // accident of what a `ProgramTest` bank happens to contain.
    let smuggled_program = Pubkey::new_from_array([0xc0; 32]);
    test.add_account(
        smuggled_program,
        Account {
            lamports: 1,
            data: Vec::from(&b"a program that is not the System Program"[..]),
            owner: bpf_loader_upgradeable::ID,
            executable: true,
            rent_epoch: 0,
        },
    );
    let mut context = test.start_with_context().await;

    // The measurement the whole clause rests on.
    let system = observe(&mut context, system_program::ID).await;
    assert!(
        system.executable,
        "the System Program account the runtime presents is executable"
    );
    // And the two accounts the clause has to tell apart really are both programs
    // to the runtime -- otherwise the refusing direction below proves nothing.
    for other in [smuggled_program, REGISTRY_PROGRAM_ID] {
        assert!(
            observe(&mut context, other).await.executable,
            "{other} must be executable for the refusing direction to be a real test"
        );
    }

    let lineage = release_lineage_address_and_bump_v1(
        &REGISTRY_PROGRAM_ID,
        predecessor.release_set_id.as_bytes(),
    )
    .0;
    let payer = context.payer.pubkey();
    let state = RegistryDeclareSuccessorState {
        payer: observe(&mut context, payer).await,
        lineage: observe(&mut context, lineage).await,
        predecessor_cache: observe(&mut context, predecessor.address).await,
        successor_cache: observe(&mut context, successor.address).await,
        system_program: system,
        rent_sysvar: observe(&mut context, sysvar::rent::ID).await,
    };
    // The host builder was always right: it projects exactly the frame conjunct
    // 6 describes. Only the program refused it.
    let report = build_registry_declare_successor_v1(REGISTRY_PROGRAM_ID, &state)
        .expect("the builder projects the devnet-shaped hop");
    let resolution = report
        .consent
        .get(ExecutionRoleV1::Resolution.role_index())
        .expect("resolution projection");
    assert!(!resolution.moved);
    assert_eq!(resolution.slot, system_program::ID);
    assert!(!resolution.must_sign);
    assert_eq!(
        report.required_signers,
        vec![payer, authority.pubkey()],
        "an unmoved role asks for no signature of its own"
    );
    let unmoved_slot = 4 + ExecutionRoleV1::Resolution.role_index();
    let moved_slot = 4 + ExecutionRoleV1::Core.role_index();

    // ---- the hostiles first, while the record's address is still pristine ----

    // What conjunct 1 protects, still protected: a program that is not the
    // System Program is refused at the frame, in an unmoved role's slot and in a
    // moved one alike, before any cache is decoded.
    for slot in [unmoved_slot, moved_slot] {
        for program in [smuggled_program, REGISTRY_PROGRAM_ID] {
            refused(
                submit(
                    &mut context,
                    perturbed(&report.instruction, |accounts| {
                        let meta = accounts.get_mut(slot).expect("consent slot");
                        meta.pubkey = program;
                        meta.is_signer = false;
                    }),
                    &[&authority],
                )
                .await,
                RegistryError::AccountFrame,
            );
        }
    }

    // The other half of conjunct 6's unmoved arm: the slot names ONE account, so
    // a stranger standing in it is refused even though nothing about it is a
    // program.
    let stranger = Keypair::new_from_array([0x6d; 32]);
    refused(
        submit(
            &mut context,
            perturbed(&report.instruction, |accounts| {
                accounts
                    .get_mut(unmoved_slot)
                    .expect("resolution consent")
                    .pubkey = stranger.pubkey();
            }),
            &[&authority],
        )
        .await,
        RegistryError::ReleaseLineageAuthorityMissing,
    );

    // Conjunct 1 conceded no decision by admitting the account: a role that DID
    // move cannot be dressed up as unmoved. Its slot carrying the System Program
    // is refused by conjunct 6, which needs a signature that account has no key
    // to produce.
    refused(
        submit(
            &mut context,
            perturbed(&report.instruction, |accounts| {
                let meta = accounts.get_mut(moved_slot).expect("core consent");
                meta.pubkey = system_program::ID;
                meta.is_signer = false;
            }),
            &[&authority],
        )
        .await,
        RegistryError::ReleaseLineageAuthorityMissing,
    );

    assert!(
        matches!(
            lineage_at(&mut context, predecessor.release_set_id).await,
            LineageAt::Undeclared
        ),
        "no refused frame may leave a record behind"
    );

    // ---- and now the hop the cut needs, end to end ----
    submit(&mut context, report.instruction.clone(), &[&authority])
        .await
        .expect("the cohort-7 to cohort-8 shape must declare");

    let landed = context
        .banks_client
        .get_account(lineage)
        .await
        .expect("banks client")
        .expect("the lineage record must exist");
    assert_eq!(landed.owner, REGISTRY_PROGRAM_ID);
    assert_eq!(landed.data.len(), RELEASE_LINEAGE_BYTES_V1);
    assert_eq!(
        landed.data,
        report.record.to_bytes(),
        "the landed record must be byte-equal to the one the builder projected"
    );
    let decoded = ReleaseLineageV1::decode(&landed.data).expect("the landed record must decode");
    assert_eq!(decoded.predecessor(), predecessor.release_set_id);
    assert_eq!(decoded.successor(), successor.release_set_id);
    for role in EXECUTION_ROLE_ORDER_V1 {
        let moved = role != ExecutionRoleV1::Resolution;
        assert_eq!(decoded.moved(role), moved, "{role:?} moved verdict");
        assert_eq!(
            decoded.consenting_authority(role),
            moved.then(|| authority.pubkey().to_bytes()),
            "an unmoved role records no consent, because none was asked for"
        );
    }

    // The walk MIGRATE runs follows this hop like any other.
    let mut chain = BTreeMap::new();
    for endpoint in [predecessor.release_set_id, successor.release_set_id] {
        chain.insert(
            endpoint.as_bytes().to_vec(),
            lineage_at(&mut context, endpoint).await,
        );
    }
    let lookup = |set: ContentId| {
        *chain
            .get(set.as_bytes().as_slice())
            .unwrap_or(&LineageAt::Undeclared)
    };
    let to_head =
        walk_lineage_to_head(predecessor.release_set_id, lookup).expect("the chain has a head");
    assert_eq!(to_head.endpoint(), successor.release_set_id);
    assert_eq!(to_head.hops(), 1);

    eprintln!(
        "four-moved hop declared at {lineage}\n  predecessor {}\n  successor   {}\n  hex {}",
        hex(predecessor.release_set_id.as_bytes()),
        hex(successor.release_set_id.as_bytes()),
        hex(&landed.data),
    );
}

/// Consent is per role, and a hop needs every moved role's own authority.
///
/// Devnet binds one key to all five roles, which makes the coalition invisible:
/// one signature satisfies every slot, and no per-slot withholding survives
/// message compilation. So this hop binds TWO authorities -- Core and Claims to
/// one, Trading, Resolution and Custody to another -- and withholds exactly the
/// second. Every other coordinate is the honest one.
#[tokio::test]
async fn every_moved_role_needs_its_own_authority() {
    let first = Keypair::new_from_array([0x71; 32]);
    let second = Keypair::new_from_array([0x72; 32]);
    let split = |specs: &mut [RoleSpec; 5]| {
        for (index, spec) in specs.iter_mut().enumerate() {
            spec.authority = Some(if index < 2 {
                first.pubkey()
            } else {
                second.pubkey()
            });
        }
    };
    let mut predecessor_specs = predecessor_specs(first.pubkey());
    split(&mut predecessor_specs);
    let mut successor_specs = all_moved_successor_specs(first.pubkey());
    split(&mut successor_specs);
    assert_eq!(
        moved_mask(predecessor_specs, successor_specs),
        [true; 5],
        "every role must move, so every slot is a consent slot"
    );

    let predecessor = build_cache(predecessor_specs);
    let successor = build_cache(successor_specs);
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    assert!(directory.join("dclutch_registry_sbf.so").exists());
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    test.add_upgradeable_program_to_genesis("dclutch_registry_sbf", &REGISTRY_PROGRAM_ID);
    test.add_account(
        predecessor.address,
        registry_account(predecessor.bytes.clone()),
    );
    test.add_account(successor.address, registry_account(successor.bytes.clone()));
    let mut context = test.start_with_context().await;

    let addresses = declare_successor_frame_addresses_v1(
        REGISTRY_PROGRAM_ID,
        predecessor.release_set_id.as_bytes(),
        successor.release_set_id.as_bytes(),
    );
    let fee_payer = context.payer.pubkey();
    let build = |second_signs: bool| {
        let mut accounts = vec![
            AccountMeta::new(fee_payer, true),
            AccountMeta::new(addresses.lineage, false),
            AccountMeta::new_readonly(predecessor.address, false),
            AccountMeta::new_readonly(successor.address, false),
        ];
        for role in EXECUTION_ROLE_ORDER_V1 {
            let index = role.role_index();
            accounts.push(if index < 2 {
                AccountMeta::new_readonly(first.pubkey(), true)
            } else {
                AccountMeta::new_readonly(second.pubkey(), second_signs)
            });
        }
        accounts.push(AccountMeta::new_readonly(system_program::ID, false));
        accounts.push(AccountMeta::new_readonly(sysvar::rent::ID, false));
        Instruction {
            program_id: REGISTRY_PROGRAM_ID,
            accounts,
            data: dclutch_registry::svm::lineage_v1::DeclareSuccessorV1::to_bytes().to_vec(),
        }
    };

    // The second authority is present and does not sign: three roles moved
    // without their consent, so the hop refuses.
    refused(
        submit(&mut context, build(false), &[&first]).await,
        RegistryError::ReleaseLineageAuthorityMissing,
    );
    assert!(
        matches!(
            lineage_at(&mut context, predecessor.release_set_id).await,
            LineageAt::Undeclared
        ),
        "a hop short one authority leaves nothing behind"
    );

    // With both, the same hop lands, and the record names each role's own
    // consenting key rather than one key for all five.
    submit(&mut context, build(true), &[&first, &second])
        .await
        .expect("both authorities consenting must land the hop");
    let landed = context
        .banks_client
        .get_account(addresses.lineage)
        .await
        .expect("banks client")
        .expect("lineage record");
    let record = ReleaseLineageV1::decode(&landed.data).expect("record decodes");
    for role in EXECUTION_ROLE_ORDER_V1 {
        let expected = if role.role_index() < 2 {
            first.pubkey()
        } else {
            second.pubkey()
        };
        assert_eq!(
            record.consenting_authority(role),
            Some(expected.to_bytes()),
            "{role:?} records the key that actually consented for it"
        );
    }
}

/// Cohort-9's shape: Custody alone stands still.
///
/// The ruling's verdict 2, made concrete. Across 8-to-9 the profile succession
/// gives Resolution V2-only reads, which is a SOURCE change to the one role
/// that did not move in 7-to-8 -- so Resolution flips to moved and Custody
/// becomes the sole unmoved role. That inversion is the point of declaring both
/// hops in one campaign: the unmoved role is not a fixed property of some role,
/// it is a per-hop measurement, and `d6e43b11`'s conjunct fix has to hold for
/// whichever role happens to hold still.
fn cohort9_specs(authority: Pubkey) -> [RoleSpec; 5] {
    let mut specs = successor_specs(authority);
    for (index, (elf, slot)) in [
        (0x91_u8, 491_000_000_u64),
        (0x92, 491_000_100),
        (0x93, 491_000_200),
        (0x94, 491_000_300),
    ]
    .into_iter()
    .enumerate()
    {
        if let Some(spec) = specs.get_mut(index) {
            spec.elf = elf;
            spec.slot = slot;
        }
    }
    specs
}

/// The generation the Registry lands in when the cut upgrades it (step 3).
const UPGRADED_REGISTRY_DEPLOYMENT_SLOT: u64 = 531;
/// A Loader V3 program is visible from `deployment_slot + 1`, and the
/// deployment slot must be an ancestor of the executing slot.
const UPGRADED_REGISTRY_BANK_SLOT: u64 = UPGRADED_REGISTRY_DEPLOYMENT_SLOT + 1;

/// Compose Loader V3 ProgramData reporting an explicit deployment generation.
fn programdata_bytes(elf: &[u8], deployment_slot: u64) -> Vec<u8> {
    let mut bytes = vec![0; 45 + elf.len()];
    bytes
        .get_mut(0..4)
        .expect("tag")
        .copy_from_slice(&3_u32.to_le_bytes());
    bytes
        .get_mut(4..12)
        .expect("slot")
        .copy_from_slice(&deployment_slot.to_le_bytes());
    // Immutable: the cut revokes the Registry's upgrade authority after the
    // move, and the declaration route never reads this account anyway.
    *bytes.get_mut(12).expect("authority tag") = 0;
    bytes.get_mut(45..).expect("ELF").copy_from_slice(elf);
    bytes
}

fn programdata_address(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

/// Declare one hop and return the record that landed.
async fn declare_hop(
    context: &mut ProgramTestContext,
    predecessor: &CacheFixture,
    successor: &CacheFixture,
    authority: &Keypair,
    funder: &Keypair,
) -> ReleaseLineageV1 {
    let lineage = release_lineage_address_and_bump_v1(
        &REGISTRY_PROGRAM_ID,
        predecessor.release_set_id.as_bytes(),
    )
    .0;
    let state = RegistryDeclareSuccessorState {
        payer: observe(context, funder.pubkey()).await,
        lineage: observe(context, lineage).await,
        predecessor_cache: observe(context, predecessor.address).await,
        successor_cache: observe(context, successor.address).await,
        system_program: observe(context, system_program::ID).await,
        rent_sysvar: observe(context, sysvar::rent::ID).await,
    };
    let report = build_registry_declare_successor_v1(REGISTRY_PROGRAM_ID, &state)
        .expect("the builder must admit an honest cut hop");
    submit(context, report.instruction.clone(), &[funder, authority])
        .await
        .expect("the upgraded Registry must admit the builder's frame");
    let landed = context
        .banks_client
        .get_account(lineage)
        .await
        .expect("banks client")
        .expect("the lineage record must exist");
    assert_eq!(landed.owner, REGISTRY_PROGRAM_ID);
    assert_eq!(landed.data.len(), RELEASE_LINEAGE_BYTES_V1);
    assert_eq!(
        landed.data,
        report.record.to_bytes(),
        "the landed record must be byte-equal to the one the builder projected"
    );
    ReleaseLineageV1::decode(&landed.data).expect("the landed record must decode")
}

/// The cut's two declarations ride the UPGRADED Registry, and one walk follows
/// both hops.
///
/// The ruling's §8.5 obligation. Every other bank in this file runs the Registry
/// at the genesis deployment generation, which is the one thing cohort-9's cut
/// guarantees will not be true when these declarations are sent: step 3 moves
/// the Registry's bytes, step 4 is the profile succession ceremony, and step 5
/// is these two hops. So the declarations are proved where they will actually
/// be made -- on a Registry whose ProgramData reports a generation that did not
/// exist when either cache was written.
///
/// That the route survives its own program moving is not free by inspection.
/// `DeclareSuccessor` reads two cache accounts and CPIs only System, so it
/// observes no deployment and has no slot pin of its own -- which is exactly
/// why it is the route that still works in the window where founding and
/// retirement do not (`found_program_test.rs`, on the brick). This test is what
/// turns that reading of the code into a measurement.
///
/// Both hops, one bank, because a lineage chain is only a chain if the hops
/// agree: `walk_lineage_to(destination)` is asked to cross BOTH, and it is
/// `walk_lineage_to` and never `is_already_current` -- the live-measured trap
/// named in `RELEASE_SET_COHORT_LINEAGE_2026_08_31.md`, where a walk that asks
/// "am I current?" answers about the wrong endpoint.
#[tokio::test]
async fn the_cut_declarations_ride_an_upgraded_registry_and_the_walk_follows_both_hops() {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    let elf = fs::read(directory.join("dclutch_registry_sbf.so"))
        .expect("compiled Registry ELF in SBF_OUT_DIR");
    assert_eq!(
        elf.get(..4),
        Some(&[0x7f, b'E', b'L', b'F'][..]),
        "the Registry artifact must be a real ELF"
    );

    let authority = authority_keypair();
    let cohort7 = build_cache(predecessor_specs(authority.pubkey()));
    let cohort8 = build_cache(successor_specs(authority.pubkey()));
    let cohort9 = build_cache(cohort9_specs(authority.pubkey()));
    // The two hops invert which role holds still, and both arms of `d6e43b11`'s
    // conjunct fix are therefore exercised in one campaign.
    assert_eq!(
        moved_mask(
            predecessor_specs(authority.pubkey()),
            successor_specs(authority.pubkey())
        ),
        [true, true, true, false, true],
        "7-to-8 is the four-moved devnet shape, resolution unmoved"
    );
    assert_eq!(
        moved_mask(
            successor_specs(authority.pubkey()),
            cohort9_specs(authority.pubkey())
        ),
        [true, true, true, true, false],
        "8-to-9 moves resolution and leaves custody alone"
    );

    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    test.add_upgradeable_program_to_genesis("dclutch_registry_sbf", &REGISTRY_PROGRAM_ID);
    // Step 3 of the cut: the Registry's bytes move to a new generation. There is
    // no Loader `Upgrade` in any harness, so the upgrade IS this restaging --
    // the same simulation `direct-hot/src/waist.rs` uses, and the only nonzero
    // deployment generation this bank holds.
    test.add_account(
        programdata_address(REGISTRY_PROGRAM_ID),
        Account {
            lamports: Rent::default().minimum_balance(45 + elf.len()),
            data: programdata_bytes(&elf, UPGRADED_REGISTRY_DEPLOYMENT_SLOT),
            owner: bpf_loader_upgradeable::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    test.add_sysvar_account(
        solana_sdk_ids::sysvar::clock::ID,
        &solana_program::clock::Clock {
            slot: UPGRADED_REGISTRY_BANK_SLOT,
            ..solana_program::clock::Clock::default()
        },
    );
    for cache in [&cohort7, &cohort8, &cohort9] {
        test.add_account(cache.address, registry_account(cache.bytes.clone()));
    }
    let funder = Keypair::new_from_array([0x2f; 32]);
    test.add_account(
        funder.pubkey(),
        Account {
            lamports: 10_000_000_000,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let mut context = test.start_with_context().await;
    context
        .warp_to_slot(UPGRADED_REGISTRY_BANK_SLOT)
        .expect("warp the bank one slot past the upgraded generation");

    // The upgrade is real to the runtime, not just to the fixture's intent.
    let observed = context
        .banks_client
        .get_account(programdata_address(REGISTRY_PROGRAM_ID))
        .await
        .expect("banks client")
        .expect("the Registry ProgramData must exist");
    assert_eq!(
        observed.data.get(4..12).map(|slot| {
            let mut bytes = [0_u8; 8];
            bytes.copy_from_slice(slot);
            u64::from_le_bytes(bytes)
        }),
        Some(UPGRADED_REGISTRY_DEPLOYMENT_SLOT),
        "the bank must be running the upgraded Registry generation"
    );

    // ---- step 5, hop one: 7 to 8, the unmoved-resolution arm ----
    let first = declare_hop(&mut context, &cohort7, &cohort8, &authority, &funder).await;
    assert_eq!(first.predecessor(), cohort7.release_set_id);
    assert_eq!(first.successor(), cohort8.release_set_id);
    assert!(
        !first.moved(ExecutionRoleV1::Resolution),
        "7-to-8 records resolution as unmoved"
    );
    assert_eq!(
        first.consenting_authority(ExecutionRoleV1::Resolution),
        None,
        "an unmoved role consents with nobody"
    );

    // ---- step 5, hop two: 8 to 9, custody now the unmoved role ----
    let second = declare_hop(&mut context, &cohort8, &cohort9, &authority, &funder).await;
    assert_eq!(second.predecessor(), cohort8.release_set_id);
    assert_eq!(second.successor(), cohort9.release_set_id);
    assert!(
        second.moved(ExecutionRoleV1::Resolution),
        "8-to-9 moves resolution, because the profile succession changed its source"
    );
    assert!(
        !second.moved(ExecutionRoleV1::Custody),
        "8-to-9 records custody as the unmoved role"
    );

    // ---- one walk, both hops ----
    let mut chain = BTreeMap::new();
    for endpoint in [
        cohort7.release_set_id,
        cohort8.release_set_id,
        cohort9.release_set_id,
    ] {
        chain.insert(
            endpoint.as_bytes().to_vec(),
            lineage_at(&mut context, endpoint).await,
        );
    }
    let lookup = |set: ContentId| {
        *chain
            .get(set.as_bytes().as_slice())
            .unwrap_or(&LineageAt::Undeclared)
    };
    let to_destination = walk_lineage_to(cohort7.release_set_id, cohort9.release_set_id, lookup)
        .expect("cohort-9 is reachable from cohort-7");
    assert_eq!(
        to_destination.hops(),
        2,
        "the walk crosses both declarations the cut makes"
    );
    // The intermediate endpoint is reachable in one, which is what makes the
    // two above a CHAIN rather than a single hop counted twice.
    let to_middle = walk_lineage_to(cohort7.release_set_id, cohort8.release_set_id, lookup)
        .expect("cohort-8 is reachable from cohort-7");
    assert_eq!(to_middle.hops(), 1);
    let to_head =
        walk_lineage_to_head(cohort7.release_set_id, lookup).expect("the chain has a head");
    assert_eq!(to_head.endpoint(), cohort9.release_set_id);
    assert_eq!(to_head.hops(), 2);
    assert!(!to_head.is_already_current());
}

/// Clone the honest instruction and change exactly one thing about its frame.
fn perturbed(honest: &Instruction, edit: impl FnOnce(&mut Vec<AccountMeta>)) -> Instruction {
    let mut instruction = honest.clone();
    edit(&mut instruction.accounts);
    instruction
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
