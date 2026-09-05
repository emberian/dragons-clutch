//! The real-ELF release waist the Registry-authenticated Hot campaign runs on.
//!
//! Extracted from `tests/registry_hot_continuation.rs`, which was its only
//! owner: every hostile Direct case a sibling test wants to add needs `elves`,
//! `add_release_waist`, `direct_case`, `direct_registry_instructions` and
//! `submit_v0`, and copying six hundred lines of waist construction into a
//! second file would be a second authority for the same fact -- it would drift
//! the first time either side moved. One owner, here, beside the chain fixture
//! these already build on.
//!
//! This is test support: it asserts and panics freely, because a fixture that
//! cannot be built has no honest value to return. The crate's `panic`,
//! `unwrap_used` and `indexing_slicing` denials are lifted for this module
//! alone, and for that reason.
#![allow(clippy::panic, clippy::unwrap_used, clippy::indexing_slicing)]

use std::{cell::Cell, collections::HashSet, env, fs, path::PathBuf};

use crate::{
    DirectHotDeploymentWidthsV5,
    chain::install_direct_hot_chain_accounts_v5,
    fixture::{
        DirectHotChainFixtureV5, DirectHotChainInputV5, DirectTradeScenarioV1,
        build_direct_hot_chain_fixture_v5,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_custody::token_svm::TokenAccount;
use dclutch_market::capability_program::hot_v3::{
    DIRECT_HOT_HEAP_FRAME_BYTES_V1, HOT_FIXED_ACCOUNT_COUNT_V3, SEALED_EXECUTION_FIXED_ALIASES_V3,
};
use dclutch_operator::hot_bump_miner::hot_bump_hint_slot_name_v1;
use dclutch_registry::release_set::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1,
    ProgramIdentityV1,
};
use dclutch_registry::svm::continuation_v1::{
    REGISTRY_CONTINUATION_REQUEST_BYTES_V1, RegistryContinuationRequestV1,
};
use dclutch_registry::svm::continuation_v2::{
    TransparentHotAdmissionSeedsV2, TransparentHotContinuationV2,
};
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1, put_activation_cache_bump_v1,
};
use dclutch_trading::native_evidence_v3::{
    DIRECT_NATIVE_EVIDENCE_BYTES_V3, encode_direct_headerless_registry_native_evidence_v4_atomic,
};
use dclutch_trading::ordinary_geometry_v3::DirectOrdinaryGeometryV3;
use solana_account::Account;
use solana_address_lookup_table_interface::state::{AddressLookupTable, LookupTableMeta};
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    clock::Clock,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{
    bpf_loader_upgradeable, compute_budget, ed25519_program, system_program, sysvar,
};
use solana_transaction::versioned::VersionedTransaction;

pub const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x91; 32]);
pub const TRADING_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x92; 32]);
pub const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x93; 32]);
pub const CLAIMS_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x94; 32]);
pub const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x95; 32]);
/// Rent program owning the sole Market-lifecycle RentCredit. It is observed,
/// never invoked, on the Direct Hot path: the adapter re-derives the credit as
/// a PDA of its own account owner and requires that owner in the frame.
pub const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x97; 32]);
pub const LOOKUP_TABLE: Pubkey = Pubkey::new_from_array([0x96; 32]);
pub const COMPUTE_LIMIT: u64 = 1_400_000;

/// The exact upgrade authority a slot-pinned substrate's ProgramData carries
/// and its `ExactAuthority` releases bind.
///
/// It signs nothing here. Its only role is to be the same 32 bytes on both
/// sides of `slot_pinned_release_elf_digest_v1`'s authority equality, so a key
/// distinct from every program identity above is exactly right.
pub const UPGRADE_AUTHORITY: Pubkey = Pubkey::new_from_array([0x9a; 32]);

/// The slot a pinned substrate's releases bind and its ProgramData reports.
///
/// Nonzero on purpose: zero is what an unpinned fixture writes, and a pin at
/// zero would pass the `u64` equality for the wrong reason. The pair below is
/// the devnet iteration runbook's own measured deploy/redeploy pair.
pub const PINNED_DEPLOYMENT_SLOT: u64 = 167;

/// The slot a SUPERSEDED substrate's ProgramData reports: strictly later than
/// [`PINNED_DEPLOYMENT_SLOT`], which is what an `Upgrade` by the bound
/// authority produces and what `slot_pin_refusal` names
/// `ReleaseSupersededByUpgrade`.
pub const UPGRADED_DEPLOYMENT_SLOT: u64 = 531;

/// The bank slot a pinned substrate whose ProgramData has never moved runs at.
///
/// # This is a runtime constraint, not a preference
///
/// A Loader V3 program is visible from `deployment_slot + 1`
/// (`DELAY_VISIBILITY_SLOT_OFFSET`), and the program cache additionally
/// requires the deployment slot to be an ANCESTOR of the executing slot in the
/// fork graph. So a bank at slot 1 can only execute a program whose ProgramData
/// reports slot 0 -- which is why the unpinned fixture writes 0, and why a
/// first attempt at a nonzero pin died in `ProgramCache::assign_program` with
/// "Unexpected replacement of an entry" rather than with anything about the
/// pin: the cache rejected an off-fork entry, reloaded it, and reloaded it
/// again until the client's deadline expired.
///
/// `PINNED_DEPLOYMENT_SLOT + 1` is exactly the first slot where the pinned
/// deployment is both effective and rooted, which is what
/// `ProgramTestContext::warp_to_slot` leaves behind.
///
/// The Direct campaign's maker replays are valid for `clock_slot ± 1`, so
/// `direct_case_v3` builds them around this slot and `start_with_substrate`
/// warps the bank to it; the two must not be set independently.
pub const PINNED_FIXTURE_BANK_SLOT: u64 = PINNED_DEPLOYMENT_SLOT + 1;

/// The bank slot the SUPERSEDED substrate runs at: one past the upgrade.
///
/// It sits above [`UPGRADED_DEPLOYMENT_SLOT`] on purpose. A superseded
/// substrate is one that WAS upgraded and now runs perfectly well; what refuses
/// is the release whose pin no longer describes it. A bank too early to load
/// the upgraded program would refuse for the wrong reason and prove nothing
/// about decision 0012.
pub const SUPERSEDED_FIXTURE_BANK_SLOT: u64 = UPGRADED_DEPLOYMENT_SLOT + 1;

/// Which release substrate the fixture stages under the same real ELFs.
///
/// # Why this exists
///
/// Decision 0012 admitted a MUTABLE substrate onto the cached-digest path:
/// `slot_pinned_release_elf_digest_v1` branches on the release's upgrade
/// policy, and the `ExactAuthority` arm -- the whole of what 0012 added -- was
/// unreachable from this fixture, because `release` built every release
/// `Immutable` and the staged ProgramData wrote the authority option as `None`.
/// Every compute figure this waist has ever produced took the `Immutable` arm.
///
/// Selected by `DCLUTCH_FIXTURE_SUBSTRATE`, in the same spirit as
/// `DCLUTCH_FIXTURE_SEED`: the default is the immutable substrate every
/// existing case was written against, so nothing moves unless a caller asks.
///
/// # The three measurable arms are not two
///
/// Comparing `Immutable` with `SlotPinned` does NOT isolate the digest arm.
/// The policy byte, the bound authority and the bound slot are all inside
/// `ArtifactReleaseV1::to_bytes`, so they move `artifact_id`, the release-set
/// identity, and therefore every PDA seeded by it -- the Registry's activation
/// cache and its Hot admission address both come off
/// `Pubkey::find_program_address` on chain. Under ledger M-61 that is a
/// REDRAWN LOTTERY, not a cost.
///
/// [`FixtureSubstrateV1::ImmutablePinned`] is the control that separates the
/// two: it keeps the `Immutable` policy and absent authority -- so it takes the
/// SAME digest arm as the default -- while binding the same nonzero slot, which
/// gives it a DIFFERENT release identity. Its distance from `Immutable` is pure
/// redraw, measured rather than assumed, and it is the yardstick the
/// `SlotPinned` distance has to be read against.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixtureSubstrateV1 {
    /// `Immutable` releases, ProgramData with authority `None` and slot 0.
    /// The pre-0012 substrate and every existing case's default.
    Immutable,
    /// `Immutable` releases pinned at [`PINNED_DEPLOYMENT_SLOT`], ProgramData
    /// with authority `None` at that slot. Same digest arm as `Immutable`, a
    /// different release identity: the redraw control.
    ImmutablePinned,
    /// `ExactAuthority` releases bound to [`UPGRADE_AUTHORITY`] and
    /// [`PINNED_DEPLOYMENT_SLOT`], ProgramData carrying exactly those.
    /// Decision 0012's arm.
    SlotPinned,
    /// The whole release set was redeployed at [`UPGRADED_DEPLOYMENT_SLOT`] by
    /// the authority it names, and every release EXCEPT Trading's was
    /// re-issued and re-pinned to the new slot. Trading's still binds
    /// [`PINNED_DEPLOYMENT_SLOT`].
    ///
    /// Only one release is superseded, on purpose. A substrate where every pin
    /// is stale would refuse too, and would prove far less: it could not
    /// distinguish "the slot pin refuses the release that moved" from "this
    /// fixture refuses". Here four pins hold and one does not, in the same
    /// transaction, against the same ProgramData accounts.
    SlotPinnedSuperseded,
}

impl FixtureSubstrateV1 {
    /// Read the substrate from `DCLUTCH_FIXTURE_SUBSTRATE`.
    ///
    /// An unset variable is [`FixtureSubstrateV1::Immutable`]. An unrecognized
    /// one PANICS rather than falling back: a sweep that silently measured the
    /// default arm while its log said otherwise would be worse than no sweep.
    pub fn from_env() -> Self {
        match env::var("DCLUTCH_FIXTURE_SUBSTRATE").ok().as_deref() {
            None | Some("") | Some("immutable") => Self::Immutable,
            Some("immutable-pinned") => Self::ImmutablePinned,
            Some("slot-pinned") => Self::SlotPinned,
            Some("slot-pinned-superseded") => Self::SlotPinnedSuperseded,
            Some(other) => panic!(
                "DCLUTCH_FIXTURE_SUBSTRATE={other}: expected immutable, \
                 immutable-pinned, slot-pinned or slot-pinned-superseded"
            ),
        }
    }

    /// The name this substrate answers to on the command line.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Immutable => "immutable",
            Self::ImmutablePinned => "immutable-pinned",
            Self::SlotPinned => "slot-pinned",
            Self::SlotPinnedSuperseded => "slot-pinned-superseded",
        }
    }

    /// The upgrade policy the staged releases carry.
    pub const fn upgrade_policy(self) -> ArtifactUpgradePolicyV1 {
        match self {
            Self::Immutable | Self::ImmutablePinned => ArtifactUpgradePolicyV1::Immutable,
            Self::SlotPinned | Self::SlotPinnedSuperseded => {
                ArtifactUpgradePolicyV1::ExactAuthority
            }
        }
    }

    /// The authority the staged releases BIND, and the one their ProgramData
    /// reports. `ArtifactReleaseV1::new` refuses any other pairing with
    /// [`FixtureSubstrateV1::upgrade_policy`].
    pub const fn upgrade_authority(self) -> Option<[u8; 32]> {
        match self {
            Self::Immutable | Self::ImmutablePinned => None,
            Self::SlotPinned | Self::SlotPinnedSuperseded => Some(UPGRADE_AUTHORITY.to_bytes()),
        }
    }

    /// The slot `program`'s staged release BINDS.
    ///
    /// Equal to [`FixtureSubstrateV1::observed_deployment_slot`] everywhere
    /// except the superseded substrate's Trading release, where the divergence
    /// IS the case: the bytes at that address moved and this release was never
    /// re-issued, so it still names the slot its activation observed.
    pub fn bound_deployment_slot(self, program: Pubkey) -> u64 {
        if self == Self::SlotPinnedSuperseded && program == TRADING_PROGRAM_ID {
            return PINNED_DEPLOYMENT_SLOT;
        }
        self.observed_deployment_slot()
    }

    /// The slot every staged ProgramData REPORTS.
    ///
    /// One value for the whole set rather than one per program, and that is a
    /// runtime constraint, not a simplification: `solana-program-test` can hold
    /// exactly one nonzero deployment generation on its fork at a time (see
    /// [`PINNED_FIXTURE_BANK_SLOT`]), so a fixture staging two live generations
    /// at once cannot load the older one at all.
    pub const fn observed_deployment_slot(self) -> u64 {
        match self {
            Self::Immutable => 0,
            Self::ImmutablePinned | Self::SlotPinned => PINNED_DEPLOYMENT_SLOT,
            Self::SlotPinnedSuperseded => UPGRADED_DEPLOYMENT_SLOT,
        }
    }

    /// The bank slot this substrate's campaign must execute at.
    ///
    /// The unpinned substrate keeps slot 1, which is where every existing case
    /// in this tree runs and what the pre-0012 CU baseline was measured at.
    pub const fn bank_slot(self) -> u64 {
        match self {
            Self::Immutable => 1,
            Self::ImmutablePinned | Self::SlotPinned => PINNED_FIXTURE_BANK_SLOT,
            Self::SlotPinnedSuperseded => SUPERSEDED_FIXTURE_BANK_SLOT,
        }
    }

    /// The slots the bank must be walked through to reach
    /// [`FixtureSubstrateV1::bank_slot`], in order.
    ///
    /// Exactly one warp, to one slot past the substrate's single deployment
    /// generation. `warp_to_slot(T)` leaves `T - 1` as the root and drops every
    /// bank below it, and the program cache admits an entry only when its
    /// deployment slot is an ancestor of the executing slot -- with
    /// `latest_root_slot` never advancing past 0 in this harness, that means
    /// the ONLY visible nonzero deployment slot is `T - 1`. Warping twice, to
    /// stage two generations at once, drops the first one off the fork and the
    /// cache reloads it forever.
    pub fn warp_slots(self) -> Vec<u64> {
        match self {
            Self::Immutable => Vec::new(),
            Self::ImmutablePinned | Self::SlotPinned => vec![PINNED_FIXTURE_BANK_SLOT],
            Self::SlotPinnedSuperseded => vec![SUPERSEDED_FIXTURE_BANK_SLOT],
        }
    }
}

/// The substrate this process stages, from the environment.
pub fn fixture_substrate() -> FixtureSubstrateV1 {
    FixtureSubstrateV1::from_env()
}

pub struct Elves {
    pub registry: Vec<u8>,
    pub trading: Vec<u8>,
    pub core: Vec<u8>,
    pub claims: Vec<u8>,
    pub custody: Vec<u8>,
}

#[derive(Clone, Copy)]
pub struct Releases {
    pub release_set: [u8; 32],
    pub activation: Pubkey,
    /// Exact bytes this function wrote at [`Releases::activation`].
    ///
    /// The account goes into the bank and nowhere else, so a fixture that
    /// hands an OPERATOR its own copy of the Hot fixed frame had no way to
    /// supply it and installed a zero-length stand-in instead. That is not
    /// inert: `activated_custody_program_v1` reads this cache for the Custody
    /// deployment, an undecodable cache yields `None`, and the mined Custody
    /// transfer-authority bump hint then comes out ABSENT where a bundle
    /// builder taking the waist's Custody program on trust mines the real one.
    /// The two envelopes differ in exactly one byte and both are valid, which
    /// is why nothing refused and only an exact cross-check could see it.
    pub activation_data: [u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1],
    pub activation_digest: [u8; 32],
    pub core_programdata: Pubkey,
    pub trading_programdata: Pubkey,
    pub claims_programdata: Pubkey,
}

pub struct DirectCase {
    pub chain: DirectHotChainFixtureV5,
    pub payer: Keypair,
    pub makers: [Keypair; 2],
}

pub fn content(value: [u8; 32]) -> ContentId {
    ContentId::new(value).expect("nonzero content identity")
}

pub fn program_identity(value: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(value.to_bytes()).expect("nonzero program identity")
}

pub fn programdata(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

pub fn elves() -> Elves {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    let read = |role: &str, name: &str| {
        let variable = format!("DCLUTCH_{}_ELF_PATH", role.to_ascii_uppercase());
        let path = match env::var_os(&variable) {
            Some(value) => {
                let path = PathBuf::from(value);
                assert!(path.is_absolute(), "{variable} must be absolute");
                let metadata = fs::symlink_metadata(&path).expect("role ELF metadata");
                assert!(
                    metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
                    "{variable} must be one regular non-symlink file"
                );
                assert_eq!(
                    fs::canonicalize(&path).expect("canonical role ELF"),
                    path,
                    "{variable} must be an exact canonical path"
                );
                path
            }
            None => directory.join(name),
        };
        fs::read(path).expect("required real ELF")
    };
    Elves {
        registry: read("registry", "dclutch_registry_sbf.so"),
        trading: read("trading", "dclutch_trading_sbf.so"),
        core: read("core", "dclutch_core_sbf.so"),
        claims: read("claims", "dclutch_claims_sbf.so"),
        custody: read("custody", "dclutch_custody_sbf.so"),
    }
}

pub fn immutable_programdata(elf: &[u8]) -> Vec<u8> {
    programdata_v2(FixtureSubstrateV1::Immutable, elf)
}

/// Stage one Loader V3 ProgramData account body for `substrate`.
///
/// The header is fixed at 45 bytes under every substrate -- variant tag,
/// deployment slot, authority option, and the 32-byte authority slot that
/// exists whether or not the option is set -- so the account WIDTH does not
/// depend on the substrate and neither do the deployment widths the chain
/// fixture derives from it. What moves is the slot at 4..12 and the option at
/// 12 with its key at 13..45, which is exactly the pair
/// `slot_pinned_release_elf_digest_v1` compares.
pub fn programdata_v2(substrate: FixtureSubstrateV1, elf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 45 + elf.len()];
    bytes
        .get_mut(..4)
        .expect("loader variant")
        .copy_from_slice(&3_u32.to_le_bytes());
    bytes
        .get_mut(4..12)
        .expect("deployment slot")
        .copy_from_slice(&substrate.observed_deployment_slot().to_le_bytes());
    match substrate.upgrade_authority() {
        None => *bytes.get_mut(12).expect("authority option") = 0,
        Some(authority) => {
            *bytes.get_mut(12).expect("authority option") = 1;
            bytes
                .get_mut(13..45)
                .expect("upgrade authority")
                .copy_from_slice(&authority);
        }
    }
    bytes.get_mut(45..).expect("ELF tail").copy_from_slice(elf);
    bytes
}

pub fn add_program(test: &mut ProgramTest, name: &'static str, program: Pubkey, elf: &[u8]) {
    add_program_v2(test, name, program, elf, fixture_substrate());
}

pub fn add_program_v2(
    test: &mut ProgramTest,
    name: &'static str,
    program: Pubkey,
    elf: &[u8],
    substrate: FixtureSubstrateV1,
) {
    test.add_upgradeable_program_to_genesis(name, &program);
    let bytes = programdata_v2(substrate, elf);
    test.add_account(
        programdata(program),
        Account {
            lamports: Rent::default().minimum_balance(bytes.len()),
            data: bytes,
            owner: bpf_loader_upgradeable::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
}

pub fn release(program: Pubkey, semantic: u8, elf: &[u8]) -> ArtifactReleaseV1 {
    release_v2(program, semantic, elf, fixture_substrate())
}

/// One artifact release on `substrate`.
///
/// The release binds the substrate's PINNED slot, never its observed one: a
/// superseded substrate is precisely a release whose bound slot no longer
/// matches what its ProgramData reports, and building the release from the
/// observation would make the pin vacuous.
pub fn release_v2(
    program: Pubkey,
    semantic: u8,
    elf: &[u8],
    substrate: FixtureSubstrateV1,
) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        program_identity(program),
        program_identity(bpf_loader_upgradeable::ID),
        programdata(program).to_bytes(),
        content([semantic; 32]),
        hash(elf).to_bytes(),
        substrate.bound_deployment_slot(program),
        substrate.upgrade_policy(),
        substrate.upgrade_authority(),
    )
    .expect("canonical slot-pinned artifact release")
}

pub fn artifact_id(value: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&value.to_bytes()).to_bytes()).expect("artifact identity")
}

pub fn binding(value: ArtifactReleaseV1) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(value.program(), artifact_id(value))
}

pub fn activation_input(value: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
    ArtifactActivationInputV1::new(
        artifact_id(value),
        value,
        DeploymentObservationV1::new(
            value.program().to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            true,
            value.programdata(),
            bpf_loader_upgradeable::ID.to_bytes(),
            false,
            value.programdata(),
            bpf_loader_upgradeable::ID.to_bytes(),
            value.deployment_slot(),
            value.elf_digest(),
            value.upgrade_authority(),
        )
        .expect("current immutable deployment observation"),
    )
}

pub fn add_release_waist(test: &mut ProgramTest, artifacts: &Elves) -> Releases {
    add_release_waist_v2(test, artifacts, fixture_substrate())
}

pub fn add_release_waist_v2(
    test: &mut ProgramTest,
    artifacts: &Elves,
    substrate: FixtureSubstrateV1,
) -> Releases {
    let core = release_v2(CORE_PROGRAM_ID, 0x31, &artifacts.core, substrate);
    let claims = release_v2(CLAIMS_PROGRAM_ID, 0x32, &artifacts.claims, substrate);
    let trading = release_v2(TRADING_PROGRAM_ID, 0x33, &artifacts.trading, substrate);
    let custody = release_v2(CUSTODY_PROGRAM_ID, 0x34, &artifacts.custody, substrate);
    let release_set = ExecutionReleaseSetV1::new(
        binding(core),
        binding(claims),
        binding(trading),
        binding(core),
        binding(custody),
    )
    .expect("Core+Trading release set");
    let release_set_id = hash(&release_set.to_bytes()).to_bytes();
    let release_set_content = content(release_set_id);
    let mut cache = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut cache, release_set_content).expect("activation cache");
    for (role, selected) in [
        (ExecutionRoleV1::Core, core),
        (ExecutionRoleV1::Claims, claims),
        (ExecutionRoleV1::Trading, trading),
        (ExecutionRoleV1::Resolution, core),
        (ExecutionRoleV1::Custody, custody),
    ] {
        activate_execution_role_into_v1(
            &mut cache,
            release_set_content,
            &release_set,
            role,
            &activation_input(selected),
        )
        .expect("activate exact role");
    }
    let (activation, activation_bump) = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set_id],
        &REGISTRY_PROGRAM_ID,
    );
    // The real Registry records this at activation, and the six readers of the
    // cache reproduce the address from it instead of searching. A fixture that
    // left it zero would stage an account no deployment produces and would
    // measure a route nobody runs.
    put_activation_cache_bump_v1(&mut cache, activation_bump).expect("activation cache bump");
    ActivatedExecutionReleaseSetV1::decode(&cache).expect("complete activation cache");
    let activation_digest = hash(&cache).to_bytes();
    let activation_data: [u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1] = cache
        .as_slice()
        .try_into()
        .expect("activation cache is exactly its declared width");
    test.add_account(
        activation,
        Account {
            lamports: Rent::default().minimum_balance(cache.len()),
            data: cache,
            owner: REGISTRY_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    Releases {
        release_set: release_set_id,
        activation,
        activation_data,
        activation_digest,
        core_programdata: programdata(CORE_PROGRAM_ID),
        trading_programdata: programdata(TRADING_PROGRAM_ID),
        claims_programdata: programdata(CLAIMS_PROGRAM_ID),
    }
}

thread_local! {
    /// In-process seed override. `None` defers to `DCLUTCH_FIXTURE_SEED`.
    static FIXTURE_SEED_OVERRIDE: Cell<Option<u64>> = const { Cell::new(None) };
}

/// Restores the previous override however [`with_fixture_seed`] leaves.
struct FixtureSeedGuard(Option<u64>);

impl Drop for FixtureSeedGuard {
    fn drop(&mut self) {
        FIXTURE_SEED_OVERRIDE.with(|cell| cell.set(self.0));
    }
}

/// Draw every fixture key from `seed` for the duration of `body`.
///
/// `DCLUTCH_FIXTURE_SEED` sweeps one seed per PROCESS, which is right for a
/// human bisecting by hand and useless for a checked-in gate that has to sweep
/// many seeds inside one `cargo test`. The obvious reach is `env::set_var`,
/// which Rust 2024 makes unsafe -- correctly, because another thread reading
/// the environment concurrently is a data race and this binary links a runtime
/// that does read it.
///
/// So the sweep does not touch the environment at all. A thread-local carries
/// the override: `#[tokio::test]` is a current-thread runtime, every fixture
/// key is drawn on that thread during the synchronous fixture build, and no
/// other thread can observe or race it. The guard restores the prior value even
/// if `body` panics, so one failing seed cannot silently reseed the next.
pub fn with_fixture_seed<R>(seed: u64, body: impl FnOnce() -> R) -> R {
    let restore = FIXTURE_SEED_OVERRIDE.with(|cell| cell.replace(Some(seed)));
    let _guard = FixtureSeedGuard(restore);
    body()
}

/// One fixture keypair, derived from a PINNED seed rather than drawn fresh.
///
/// # Why this is not `Keypair::new()` any more
///
/// It was, and that made every compute figure this fixture produces a DRAW
/// rather than a measurement. The Hot path derives program addresses whose
/// seeds include these keys, and `try_find_program_address` costs 1,500 CU per
/// attempt, so the bump-search depth for a given set of keys is worth thousands
/// of compute units. Measured (W2p, 2026-08-27) on ONE ELF, fifteen runs: the
/// same bytes consumed anywhere from 1,342,859 to 1,386,358 CU -- a spread of
/// 43,499 against a 1,400,000 ceiling. The lane that met that spread before
/// recorded it as "codegen noise of +-20,000 CU between builds of the same
/// source", which it is not: it is this, and it does not need two builds.
///
/// Pinning makes a single figure mean something. It does NOT make the spread go
/// away -- on a real chain the makers are whoever they are, so that spread is a
/// property of the protocol and a gate margin has to cover the WORST keys, not
/// these. Sweep it with `DCLUTCH_FIXTURE_SEED=<n>`, which redraws every fixture
/// key deterministically; the default is seed 0.
///
/// Any 32 bytes are a valid ed25519 secret key -- the signing scalar is derived
/// by hashing them -- so a low-entropy seed still produces a public key that is
/// a fair sample for a bump search.
fn fixture_keypair(role: u8) -> Keypair {
    let seed = FIXTURE_SEED_OVERRIDE
        .with(Cell::get)
        .or_else(|| {
            env::var("DCLUTCH_FIXTURE_SEED")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
        })
        .unwrap_or(0);
    let mut secret = [0_u8; 32];
    secret[0] = role;
    secret[1..9].copy_from_slice(&seed.to_le_bytes());
    Keypair::new_from_array(secret)
}

pub fn direct_case(
    test: &mut ProgramTest,
    releases: Releases,
    artifacts: &Elves,
    corrupt_destination: bool,
) -> DirectCase {
    direct_case_v2(test, releases, artifacts, corrupt_destination, false)
}

/// Build the canonical Direct case, optionally leaving the seal PDA vacant.
///
/// The ordinary campaign installs the seal already written, exactly as a Market
/// that has sealed this closure once would find it. `vacant_seal` leaves the
/// PDA empty and System-owned instead, which is the prestate the on-chain seal
/// outer requires.
pub fn direct_case_v2(
    test: &mut ProgramTest,
    releases: Releases,
    artifacts: &Elves,
    corrupt_destination: bool,
    vacant_seal: bool,
) -> DirectCase {
    direct_case_v3(
        test,
        releases,
        artifacts,
        corrupt_destination,
        vacant_seal,
        fixture_substrate(),
    )
}

/// [`direct_case_v2`] against an explicitly named substrate.
///
/// The substrate reaches this function only through the ProgramData WIDTHS it
/// implies, which are the same under all four; the assertion below is what
/// keeps that true rather than assumed.
pub fn direct_case_v3(
    test: &mut ProgramTest,
    releases: Releases,
    artifacts: &Elves,
    corrupt_destination: bool,
    vacant_seal: bool,
    substrate: FixtureSubstrateV1,
) -> DirectCase {
    direct_case_v4(
        test,
        releases,
        artifacts,
        corrupt_destination,
        vacant_seal,
        substrate,
        DirectOrdinaryGeometryV3::CANONICAL,
    )
}

/// [`direct_case_v3`] at an explicitly named market geometry.
///
/// The geometry moves the market's Product, Claims and Position records and
/// the transaction's Product tail count. It does NOT move the Direct
/// artifacts: their runtime-width coordinates are affine rules the executor
/// resolves against that tail, so the same descriptor, the same seal and the
/// same content identities serve every geometry. The canonical three-outcome
/// demo is one cut; the journey's four-outcome market is two.
pub fn direct_case_v4(
    test: &mut ProgramTest,
    releases: Releases,
    artifacts: &Elves,
    corrupt_destination: bool,
    vacant_seal: bool,
    substrate: FixtureSubstrateV1,
    geometry: DirectOrdinaryGeometryV3,
) -> DirectCase {
    direct_case_v5(
        test,
        releases,
        artifacts,
        corrupt_destination,
        vacant_seal,
        substrate,
        geometry,
        DirectTradeScenarioV1::ZERO_FEE,
    )
}

/// [`direct_case_v4`] executing an explicitly named trade.
///
/// # The scenario is the difference between one Custody route and two
///
/// Every other argument to this function moves identities, widths or geometry.
/// This one moves what the transaction DOES: the transition derives the four
/// Custody routes' enable registers from the combined fee, so a trade whose fee
/// floors to zero projects exactly one live route and a trade whose fee does
/// not projects two. `DirectTradeScenarioV1::ZERO_FEE` is the trade every CU
/// figure in this repository was measured on, and it is the floor case, not the
/// ordinary one: a market that charges a fee runs the other shape.
///
/// The scenario reaches the chain only through the fixture -- the five role
/// ELFs are untouched by it -- so two sweeps that differ only in this argument
/// are a CONTROLLED pair, with identical programs and identical bump depths at
/// every site whose seeds the scenario does not enter.
/// The exact chain-fixture input `direct_case_v5` builds its case from.
///
/// Extracted so a probe that needs the fixture's own derived values -- the
/// projected Custody legs, say -- states the same input the case states rather
/// than a second copy of it. Draws the payer and both makers from the ambient
/// fixture seed, so it must be called inside the same
/// [`with_fixture_seed`] scope as the case it describes.
pub fn direct_chain_input_v5(
    releases: Releases,
    artifacts: &Elves,
    substrate: FixtureSubstrateV1,
    geometry: DirectOrdinaryGeometryV3,
    trade: DirectTradeScenarioV1,
) -> DirectHotChainInputV5 {
    let payer = fixture_keypair(0);
    let makers = [fixture_keypair(1), fixture_keypair(2)];
    // Substrate-invariant by construction: Loader V3's metadata header is 45
    // bytes whether or not the authority option is set, so every substrate
    // stages the same account WIDTH and the chain fixture derives the same
    // deployment widths. Asserted below rather than assumed, because a width
    // that silently moved with the substrate would put the CU difference
    // between two sweeps into account data instead of into the digest arm --
    // which is exactly the confound these arms exist to separate.
    let deployment_widths = DirectHotDeploymentWidthsV5::new(
        programdata_v2(substrate, &artifacts.trading).len(),
        programdata_v2(substrate, &artifacts.claims).len(),
        programdata_v2(substrate, &artifacts.core).len(),
    )
    .expect("real Direct deployment widths");
    assert_eq!(
        deployment_widths,
        DirectHotDeploymentWidthsV5::new(
            immutable_programdata(&artifacts.trading).len(),
            immutable_programdata(&artifacts.claims).len(),
            immutable_programdata(&artifacts.core).len(),
        )
        .expect("immutable Direct deployment widths"),
        "the staged substrate changed a ProgramData account width",
    );
    DirectHotChainInputV5 {
        registry_program: REGISTRY_PROGRAM_ID,
        trading_program: TRADING_PROGRAM_ID,
        core_program: CORE_PROGRAM_ID,
        claims_program: CLAIMS_PROGRAM_ID,
        custody_program: CUSTODY_PROGRAM_ID,
        rent_program: RENT_PROGRAM_ID,
        release_set: releases.release_set,
        activation_cache: releases.activation,
        trading_programdata: releases.trading_programdata,
        core_programdata: releases.core_programdata,
        claims_programdata: releases.claims_programdata,
        deployment_widths,
        payer: payer.pubkey(),
        makers: [makers[0].pubkey(), makers[1].pubkey()],
        clock_slot: substrate.bank_slot(),
        geometry,
        // `add_release_waist` binds Trading at semantic release 0x33; the
        // validated-artifact seal is filed under exactly that release.
        trading_semantic_release: [0x33; 32],
        trade,
    }
}

pub fn direct_case_v5(
    test: &mut ProgramTest,
    releases: Releases,
    artifacts: &Elves,
    corrupt_destination: bool,
    vacant_seal: bool,
    substrate: FixtureSubstrateV1,
    geometry: DirectOrdinaryGeometryV3,
    trade: DirectTradeScenarioV1,
) -> DirectCase {
    let payer = fixture_keypair(0);
    let makers = [fixture_keypair(1), fixture_keypair(2)];
    // The maker replays this fixture signs are valid for `clock_slot +- 1` and
    // `hot_v3` reads the live `Clock`, so the bank has to BE at this slot when
    // the campaign submits. `start_with_substrate` is the other half; calling
    // `ProgramTest::start_with_context` directly on a pinned substrate leaves
    // the bank at slot 1, where the pinned programs are not yet visible.
    let clock = Clock {
        slot: substrate.bank_slot(),
        ..Clock::default()
    };
    test.add_sysvar_account(sysvar::clock::ID, &clock);
    test.add_account(
        payer.pubkey(),
        Account {
            lamports: 10_000_000_000,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    let input = direct_chain_input_v5(releases, artifacts, substrate, geometry, trade);
    let hand =
        build_direct_hot_chain_fixture_v5(input).expect("canonical Profile14 Direct chain fixture");
    // The campaign executes the BUILDER's bundle; the hand-built fixture rides
    // along as a checked oracle, so every gate run is also a reproduction run.
    let mut chain =
        crate::fixture::via_builder::build_direct_hot_chain_fixture_via_builder_v1(input)
            .expect("artifact-derived Direct chain bundle");
    assert_builder_reproduces_hand(&chain, &hand);
    if corrupt_destination {
        let destination = chain.collateral_accounts[1];
        let account = chain
            .accounts
            .iter_mut()
            .find(|value| value.key == destination)
            .expect("Custody destination fixture account");
        let state = account
            .account
            .data
            .get_mut(108)
            .expect("base token state byte");
        *state = 0;
        assert!(TokenAccount::parse(&account.account.data).is_ok());
    }
    if vacant_seal {
        let seal = chain.capability_seal;
        let account = chain
            .accounts
            .iter_mut()
            .find(|value| value.key == seal)
            .expect("validated-artifact seal fixture account");
        account.account.data = Vec::new();
        account.account.owner = system_program::ID;
        account.account.lamports = 0;
    }
    for (index, candidate) in chain.accounts.iter().enumerate() {
        if candidate.key == Pubkey::default() {
            assert_eq!(candidate.key, system_program::ID);
            assert!(chain.externally_installed_keys.contains(&candidate.key));
        }
        let prior = chain
            .accounts
            .get(..index)
            .and_then(|accounts| accounts.iter().position(|other| other.key == candidate.key));
        assert!(
            prior.is_none(),
            "Direct fixture account {index} aliases account {prior:?}: {}",
            candidate.key
        );
    }
    let installed = install_direct_hot_chain_accounts_v5(
        test,
        &Rent::default(),
        &chain.accounts,
        &chain.externally_installed_keys,
    )
    .expect("install canonical Direct-owned accounts");
    assert_eq!(
        installed.rollback_snapshot_keys,
        chain.rollback_snapshot_keys
    );
    DirectCase {
        chain,
        payer,
        makers,
    }
}

/// The builder's bundle is the hand-built one, byte for byte.
///
/// External-key order is the one normalized comparison: both sides consume
/// that list only through `contains`.
fn assert_builder_reproduces_hand(built: &DirectHotChainFixtureV5, hand: &DirectHotChainFixtureV5) {
    assert_hot_instructions_agree(&built.hot_instruction, &hand.hot_instruction);
    assert_eq!(built.signed_messages, hand.signed_messages);
    assert_eq!(built.accounts, hand.accounts);
    assert_eq!(built.rollback_snapshot_keys, hand.rollback_snapshot_keys);
    assert_eq!(built.market, hand.market);
    assert_eq!(built.root, hand.root);
    assert_eq!(built.claims_market, hand.claims_market);
    assert_eq!(built.claims_positions, hand.claims_positions);
    assert_eq!(built.maker_replays, hand.maker_replays);
    assert_eq!(built.custody_replay, hand.custody_replay);
    assert_eq!(built.collateral_accounts, hand.collateral_accounts);
    assert_eq!(built.custody_routes, hand.custody_routes);
    assert_eq!(built.capability_seal, hand.capability_seal);
    assert_eq!(built.capability_seal_bytes, hand.capability_seal_bytes);
    assert_eq!(built.descriptor_digest, hand.descriptor_digest);
    let mut built_external = built.externally_installed_keys.clone();
    let mut hand_external = hand.externally_installed_keys.clone();
    built_external.sort_unstable_by_key(Pubkey::to_bytes);
    hand_external.sort_unstable_by_key(Pubkey::to_bytes);
    assert_eq!(built_external, hand_external);
}

/// The two Hot instructions agree, and when they do not, WHERE.
///
/// # Why this is not one `assert_eq!`
///
/// It was, and the cost was measured. This instruction carries eighty account
/// metas and 584 bytes of data, so a one-byte disagreement printed two
/// twelve-kilobyte `Debug` renderings and left the reader to diff them by eye
/// -- across 52 failing rows in thirteen binaries. That is `map_err(|_| Coarse)`
/// in an assertion: the comparison knew exactly which offset moved and threw
/// the fact away. Three bytes of 584 were the whole disagreement, and finding
/// that out took a diff of the panic text rather than a read of it.
///
/// So: report the first disagreement in the terms of the thing that
/// disagreed -- a meta index, or a list of byte offsets with both values, with
/// the envelope FIELD named when the offset lands inside the bump-hint block.
fn assert_hot_instructions_agree(built: &Instruction, hand: &Instruction) {
    assert_eq!(
        built.program_id, hand.program_id,
        "builder and hand Hot instructions name different programs"
    );
    assert_eq!(
        built.accounts.len(),
        hand.accounts.len(),
        "builder Hot instruction has {} account metas, hand-built has {}",
        built.accounts.len(),
        hand.accounts.len()
    );
    for (index, (left, right)) in built.accounts.iter().zip(hand.accounts.iter()).enumerate() {
        assert_eq!(
            left, right,
            "builder and hand Hot instructions differ at account meta {index}: \
             builder {left:?}, hand {right:?}"
        );
    }
    assert_eq!(
        built.data.len(),
        hand.data.len(),
        "builder Hot request is {} bytes, hand-built is {}",
        built.data.len(),
        hand.data.len()
    );
    let differing = built
        .data
        .iter()
        .zip(hand.data.iter())
        .enumerate()
        .filter(|(_, (left, right))| left != right)
        .map(|(offset, (left, right))| {
            format!(
                "{offset}{} builder {left} hand {right}",
                envelope_field(offset)
            )
        })
        .collect::<Vec<_>>();
    assert!(
        differing.is_empty(),
        "builder and hand Hot request data differ in {} of {} bytes: {}",
        differing.len(),
        built.data.len(),
        differing.join("; ")
    );
}

/// Name the envelope field an offset lands in, for the fields whose
/// disagreement has a specific author.
///
/// Only the caller-mined bump hints are named. They are the block a producer
/// fills and a fixture can silently leave zero -- which is exactly how 52 rows
/// came to fail on three bytes -- and an offset inside them points at a
/// derivation rather than at a digest.
///
/// The names are READ from `HOT_BUMP_HINT_SLOT_NAMES_V1`, which the producer
/// owns, rather than spelled here. This function used to spell them, and its
/// copy had already drifted from the producer's in a way that cost the reader
/// the part that mattered: it merged `lifecycle[0]` with `lifecycle[1]` and
/// both `child_caller` and both `child_relay` slots into one name each, so a
/// disagreement at exactly one of a pair reported the pair. A value duplicated
/// instead of read agrees right up until it does not.
fn envelope_field(offset: usize) -> String {
    hot_bump_hint_slot_name_v1(offset)
        .map(|name| format!(" (HotBumpHintsV1::{name})"))
        .unwrap_or_default()
}

pub fn registry_hot_instruction(releases: Releases, mut hot: Instruction) -> (Instruction, Pubkey) {
    assert_eq!(hot.program_id, TRADING_PROGRAM_ID);
    assert!(hot.accounts.len() >= HOT_FIXED_ACCOUNT_COUNT_V3);
    let cache_digest = content(releases.activation_digest);
    let hot_digest = content(hash(&hot.data).to_bytes());
    let continuation = TransparentHotContinuationV2::new(
        content(releases.release_set),
        cache_digest,
        hot_digest,
        u32::try_from(hot.data.len()).expect("Hot width"),
    )
    .expect("transparent Core+Trading Hot continuation");
    let batch = continuation.role_batch_request().expect("role batch");
    let seeds = TransparentHotAdmissionSeedsV2::new(
        continuation,
        releases.activation.to_bytes(),
        content(hash(&batch.to_bytes()).to_bytes()),
    )
    .expect("admission seeds");
    let release = seeds.release_set();
    let cache = seeds.activation_cache();
    let batch = seeds.batch_request_digest();
    let mask = seeds.role_mask();
    let role = seeds.continuation_role();
    let digest = seeds.hot_instruction_digest();
    let admission = Pubkey::find_program_address(
        &[
            seeds.domain(),
            release.as_slice(),
            cache.as_slice(),
            batch.as_slice(),
            mask.as_slice(),
            role.as_slice(),
            digest.as_slice(),
        ],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    hot.accounts.insert(
        HOT_FIXED_ACCOUNT_COUNT_V3,
        AccountMeta::new_readonly(admission, false),
    );
    let mut accounts = vec![
        AccountMeta::new_readonly(releases.activation, false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(releases.core_programdata, false),
        AccountMeta::new_readonly(TRADING_PROGRAM_ID, false),
        AccountMeta::new_readonly(releases.trading_programdata, false),
        AccountMeta::new_readonly(admission, false),
    ];
    accounts.extend(hot.accounts);
    (
        Instruction {
            program_id: REGISTRY_PROGRAM_ID,
            accounts,
            data: hot.data,
        },
        admission,
    )
}

pub fn legacy_registry_hot_instruction(
    releases: Releases,
    hot: Instruction,
) -> (Instruction, Pubkey) {
    let (mut outer, admission) = registry_hot_instruction(releases, hot);
    let request = RegistryContinuationRequestV1::new_core_trading_hot(
        content(releases.release_set),
        content(releases.activation_digest),
        content(hash(&outer.data).to_bytes()),
        u32::try_from(outer.data.len()).expect("Hot width"),
    )
    .expect("legacy headered continuation");
    let mut data = Vec::with_capacity(REGISTRY_CONTINUATION_REQUEST_BYTES_V1 + outer.data.len());
    data.extend_from_slice(&request.to_bytes());
    data.extend_from_slice(&outer.data);
    outer.data = data;
    (outer, admission)
}

/// The canonical continuation transaction: budget, heap, evidence, outer.
///
/// # THE HEAP REQUEST, AND WHY IT IS HERE NOW
///
/// It was absent, and its absence was the whole of
/// `registry_hot_continuation`'s five reds. The control died
/// `InstructionError(2, ProgramFailedToComplete)` with Trading logging
/// "memory allocation failed, out of memory": this route needs about 33,020
/// bytes and the protocol default is 32,768, and the allocation that crosses
/// that line is infallible, so it ABORTS instead of refusing `HeapExhausted`.
/// Three hostiles then asserted refusals against an abort and proved nothing,
/// which is ledger M-38 exactly.
///
/// Two claims used to stand against putting one here and both have expired:
///
/// - "it fits the protocol default". It did, at `365304c2d`'s measured 29,895
///   of 32,768 on 2026-09-01. It does not now.
/// - "its packet has four spare bytes and could not carry one anyway". That was
///   true when the packet was 1,228. `74e044cf3` moved the System program into
///   the lookup table for 31 bytes and the alias projection took six more
///   before it, so the packet is 1,167 and the spare is SIXTY-FIVE. A
///   `RequestHeapFrame` costs eight -- one program-id index, one empty
///   account-index vector, and five bytes of data behind a length byte -- and
///   its program id is already a static key because the instruction above it is
///   a ComputeBudget one. 1,175, with fifty-seven still spare.
///
/// Nothing is DECLARED by this that was not already: `DCLTHOT3` has been on
/// `declares_extended_heap_profile_v1`'s list since `8ee544e4`, and
/// `registry/hot_continuation_v2::process` forwards the Hot bytes unchanged, so
/// the declaration cannot tell the two routes apart and never could. What was
/// missing was the GRANT, which is the thing the adapter reads out of the
/// instructions sysvar and the thing a route actually spends. Declaring makes a
/// grant admissible; only asking for one makes it arrive.
///
/// # The index, which is not free to move
///
/// It goes second, for the same pair of reasons the top-level frame's does. It
/// cannot go last: the runtime clears return data at the start of every
/// top-level instruction, so a trailing ComputeBudget instruction erases the
/// commit-last acknowledgement the Hot execution just produced. It cannot go
/// silently in front either: the native-signature evidence names the index it
/// expects the signed Registry bytes at, so the evidence is encoded for 3 here
/// rather than left at a stale 2.
///
/// # A caller must not force the bank's budget
///
/// `ProgramTest::set_compute_max_units` installs one fixed budget and makes the
/// runtime ignore the transaction's own ComputeBudget instructions, this
/// request included -- while the adapter still reads the sysvar and lifts its
/// ceiling over a mapping the runtime never widened. Use
/// `program_test_without_forced_budget`; the `SetComputeUnitLimit` below is
/// what a real transaction pays with and is carried for exactly that reason.
pub fn direct_registry_instructions(releases: Releases, direct: &DirectCase) -> [Instruction; 4] {
    let (registry, _) = registry_hot_instruction(releases, direct.chain.hot_instruction.clone());
    let signatures = [
        direct.makers[0]
            .sign_message(&direct.chain.signed_messages[0])
            .as_ref()
            .try_into()
            .expect("seller signature width"),
        direct.makers[1]
            .sign_message(&direct.chain.signed_messages[1])
            .as_ref()
            .try_into()
            .expect("buyer signature width"),
    ];
    let mut evidence = [0_u8; DIRECT_NATIVE_EVIDENCE_BYTES_V3];
    encode_direct_headerless_registry_native_evidence_v4_atomic(
        3,
        &registry.data,
        signatures,
        &mut evidence,
    )
    .expect("detached current-Registry native evidence");
    [
        Instruction {
            program_id: compute_budget::ID,
            accounts: Vec::new(),
            data: {
                let mut data = vec![2];
                data.extend_from_slice(
                    &u32::try_from(COMPUTE_LIMIT)
                        .expect("compute limit width")
                        .to_le_bytes(),
                );
                data
            },
        },
        Instruction {
            program_id: compute_budget::ID,
            accounts: Vec::new(),
            data: {
                let mut data = vec![1];
                data.extend_from_slice(&DIRECT_HOT_HEAP_FRAME_BYTES_V1.to_le_bytes());
                data
            },
        },
        Instruction {
            program_id: ed25519_program::ID,
            accounts: Vec::new(),
            data: evidence.to_vec(),
        },
        registry,
    ]
}

/// The same Direct trade, submitted TOP-LEVEL to Trading.
///
/// This is how every public caller sends one: no Registry outer, no admission
/// PDA, just the bare Hot instruction on the canonical fixed frame. Trading
/// takes its `ReauthenticateRegistry` arm and asks the Registry to
/// re-authenticate Core and itself by CPI, rather than being handed a
/// continuation's receipts.
///
/// The native evidence is byte-for-byte the evidence the continuation form
/// uses. It can be, because the container is already `TradingHot` with
/// Hot-relative coordinates and the Registry outer's data is byte-identical to
/// the Hot data it wraps -- so the makers sign the same message either way, at
/// the same instruction index. Nothing about the makers' intent changes with
/// the route; only who is invoked first does.
pub fn direct_top_level_instructions(direct: &DirectCase) -> [Instruction; 4] {
    direct_top_level_instructions_with_signatures(
        direct,
        [
            direct.makers[0]
                .sign_message(&direct.chain.signed_messages[0])
                .as_ref()
                .try_into()
                .expect("seller signature width"),
            direct.makers[1]
                .sign_message(&direct.chain.signed_messages[1])
                .as_ref()
                .try_into()
                .expect("buyer signature width"),
        ],
    )
}

/// The same four instructions, over signatures the CALLER brings.
///
/// [`direct_top_level_instructions`] signs the fixture's own preimages with the
/// fixture's own keypairs, which is the right default and is also the one thing
/// a ticket test must not do: a signature produced two lines above the
/// transaction proves nothing about a signature that travelled through a JSON
/// file. `ticket_authored_intents_execute_top_level.rs` reads two authored
/// ticket files and hands their detached signatures here, so the bytes the
/// Ed25519 program verifies are the bytes a maker's ticket carried.
///
/// Everything downstream of the signatures -- the evidence encoding, the
/// instruction index it names, the heap grant and its position -- stays here,
/// with one author.
pub fn direct_top_level_instructions_with_signatures(
    direct: &DirectCase,
    signatures: [[u8; 64]; 2],
) -> [Instruction; 4] {
    // The heap grant rides AHEAD of the evidence, and the Hot instruction is
    // therefore at index 3 rather than 2. Both positions are forced, and by
    // opposite constraints:
    //
    // - it cannot go last. The runtime clears return data at the start of every
    //   top-level instruction, so a trailing ComputeBudget instruction erases
    //   the commit-last ACK the Hot execution just produced -- the transaction
    //   succeeds and its evidence is gone.
    // - it cannot go silently in front either. The native-signature evidence
    //   names the index it expects to find the signed Hot instruction at, so
    //   moving Hot means re-encoding the evidence for its new index. That is
    //   what this does, rather than leaving a stale 2 behind.
    //
    // The continuation route carries one too, as of 2026-09-03, and used to
    // carry none because it fit the protocol default. It stopped fitting: see
    // `direct_registry_instructions`. This route needs its own for a separate
    // reason that has not changed -- it makes two Registry reauthentication CPIs
    // the other never makes and holds their frames against an allocator that
    // never frees.
    let mut evidence = [0_u8; DIRECT_NATIVE_EVIDENCE_BYTES_V3];
    encode_direct_headerless_registry_native_evidence_v4_atomic(
        3,
        &direct.chain.hot_instruction.data,
        signatures,
        &mut evidence,
    )
    .expect("detached native evidence over the top-level Hot instruction");
    [
        Instruction {
            program_id: compute_budget::ID,
            accounts: Vec::new(),
            data: {
                let mut data = vec![2];
                data.extend_from_slice(
                    &u32::try_from(COMPUTE_LIMIT)
                        .expect("compute limit width")
                        .to_le_bytes(),
                );
                data
            },
        },
        Instruction {
            program_id: compute_budget::ID,
            accounts: Vec::new(),
            data: {
                let mut data = vec![1];
                data.extend_from_slice(&DIRECT_HOT_HEAP_FRAME_BYTES_V1.to_le_bytes());
                data
            },
        },
        Instruction {
            program_id: ed25519_program::ID,
            accounts: Vec::new(),
            data: evidence.to_vec(),
        },
        direct.chain.hot_instruction.clone(),
    ]
}

/// The canonical lookup-table address set, from its ONE author.
///
/// This used to be a second copy of `dclutch_operator::direct_inline_v3::
/// canonical_lookup_addresses`, and the copies did not agree: this one filtered
/// `meta.is_signer` PER OCCURRENCE, so an account signing in one instruction and
/// not in another entered the table through its non-signing appearance -- and a
/// v0 message must carry every signer as a static key. The fixtures had not
/// reached a frame shaped that way, which is the only reason it had not bitten.
///
/// The signer set is collected from the metas here because a test harness has no
/// separate declaration of it; the FILTER is the operator's.
pub fn canonical_lookup_addresses(instructions: &[Instruction], payer: Pubkey) -> Vec<Pubkey> {
    // `Pubkey::default()` is how a caller here says "no payer chosen yet" -- a
    // table is staged before the harness picks who pays -- so it is not added to
    // the signer set rather than being passed off as one.
    let mut signers = Vec::new();
    if payer != Pubkey::default() {
        signers.push(payer);
    }
    for meta in instructions
        .iter()
        .flat_map(|instruction| &instruction.accounts)
    {
        if meta.is_signer && !signers.contains(&meta.pubkey) {
            signers.push(meta.pubkey);
        }
    }
    dclutch_operator::direct_inline_v3::canonical_lookup_addresses_excluding_signers(
        instructions,
        &signers,
    )
    .expect("canonical Direct lookup-table addresses")
}

/// The superseded per-occurrence derivation, kept only to be compared against.
///
/// `canonical_lookup_addresses_disagreement_is_measured_not_assumed` drives both
/// on the real registered creation frames, so "the copies agreed on this fixture"
/// is a measurement rather than a hope, and the day a frame makes them disagree
/// the case says so instead of the message quietly gaining a non-static signer.
#[cfg(test)]
pub(crate) fn superseded_per_occurrence_lookup_addresses(
    instructions: &[Instruction],
    payer: Pubkey,
) -> Vec<Pubkey> {
    let programs = instructions
        .iter()
        .map(|instruction| instruction.program_id)
        .collect::<Vec<_>>();
    let mut addresses = instructions
        .iter()
        .flat_map(|instruction| &instruction.accounts)
        .filter(|meta| !meta.is_signer && meta.pubkey != payer && !programs.contains(&meta.pubkey))
        .map(|meta| meta.pubkey)
        .collect::<Vec<_>>();
    addresses.sort_unstable_by_key(Pubkey::to_bytes);
    addresses.dedup();
    addresses
}

/// The canonical lookup-table account for one address set.
///
/// One author for the encoding, because there are two ways to put a table in
/// front of a bank -- at genesis and between two transactions of a same-bank
/// campaign -- and a campaign that re-derived the second would be a second
/// authority for the table's own serialization.
#[must_use]
pub fn lookup_table_account(addresses: &[Pubkey]) -> Account {
    let data = AddressLookupTable {
        meta: LookupTableMeta::default(),
        addresses: addresses.into(),
    }
    .serialize_for_tests()
    .expect("lookup-table bytes");
    Account {
        lamports: Rent::default().minimum_balance(data.len()),
        data,
        owner: solana_address_lookup_table_interface::program::id(),
        executable: false,
        rent_epoch: 0,
    }
}

pub fn add_lookup_table(test: &mut ProgramTest, addresses: &[Pubkey]) {
    test.add_account(LOOKUP_TABLE, lookup_table_account(addresses));
}

/// Replace the live bank's lookup table between two transactions.
///
/// A multi-action campaign signs one account list per action and they differ,
/// so the table each message resolves through differs too. `LookupTableMeta`
/// defaults to a zero `last_extended_slot`, which is what makes a table written
/// this way usable in the very next transaction rather than the next slot.
///
/// This is HARNESS TRANSPORT and nothing else: the table names no protocol
/// fact, and every account it carries is already derived by the builder and
/// checked against the bank by the campaign's own frame control.
pub fn set_lookup_table(context: &mut ProgramTestContext, addresses: &[Pubkey]) {
    context.set_account(
        &LOOKUP_TABLE,
        &solana_account::AccountSharedData::from(lookup_table_account(addresses)),
    );
}

/// Start the bank and put it where this substrate's pins require it.
///
/// A `ProgramTest` always starts at slot 1. That is correct for the unpinned
/// substrate and unusable for a pinned one: a Loader V3 program becomes visible
/// one slot after its ProgramData's deployment slot, so at slot 1 a pin at 167
/// makes every program in the fixture invisible and the runtime reports it as a
/// cache replacement, not as anything to do with the pin.
///
/// This is the ONLY correct way to start a pinned case, and it is paired with
/// `direct_case_v3`'s clock: both read [`FixtureSubstrateV1::bank_slot`], so
/// the signed maker replays and the bank agree by construction.
pub async fn start_with_substrate(
    test: ProgramTest,
    substrate: FixtureSubstrateV1,
) -> ProgramTestContext {
    let mut context = test.start_with_context().await;
    for slot in substrate.warp_slots() {
        context
            .warp_to_slot(slot)
            .expect("warp the bank to the substrate's pinned slot");
    }
    context
}

pub async fn submit_v0(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    addresses: Vec<Pubkey>,
    transaction_payer: Option<&Keypair>,
    signers: &[&Keypair],
) -> Result<u64, RefusedExecution> {
    Ok(
        submit_v0_observed(context, instructions, addresses, transaction_payer, signers)
            .await?
            .compute_units_consumed,
    )
}

/// Emit one census observation, when the gauntlet asked for evidence.
///
/// A no-op in an ordinary `cargo test` run, because
/// `dclutch_program_test_evidence::evidence_directory` is `None` with the
/// environment variable unset. This is the ONLY place in the Direct family that
/// can record one: it is the single funnel every suite's submission passes
/// through, and it is the only frame that knows the signature, the observed
/// slot and the exact wire width at once.
///
/// **THE WIRE WIDTH IS THE POINT AS MUCH AS THE ROUTE IS.**
/// `docs/design/PACKET_LIMIT_2026_09_01.md` records that no C-04 program-test
/// reports `wire_bytes`, and a campaign that does not measure the packet cannot
/// claim the fast lane's second condition -- ProgramTest submits no packet, so
/// it enforces no 1,232-byte maximum and an over-wide frame survives untouched.
/// This function has the number the assertion above already computed, so it
/// reports it rather than passing `None`.
///
/// The label is DERIVED, never hand-written, for the reason the Dealer campaign
/// gives at length: a label is the key a `bindings.json` row matches on, so a
/// hand-written one can drift from the transaction it names while both still
/// look right. This one is the libtest thread name -- the test's own path --
/// plus that test's submission ordinal, plus a disposition READ BACK from what
/// the runtime reported. It cannot name a transaction other than its own, a
/// second submission in one test cannot collide with the first, and a refusal
/// that changes code changes binding instead of quietly reusing one.
fn record_campaign_transaction(
    signature: &str,
    slot: u64,
    wire_bytes: usize,
    logs: &[String],
    result: &Result<(), solana_sdk::transaction::TransactionError>,
    compute_units_consumed: Option<u64>,
) {
    use solana_sdk::instruction::InstructionError;
    use solana_sdk::transaction::TransactionError;

    if dclutch_program_test_evidence::evidence_directory().is_none() {
        return;
    }
    thread_local! {
        static ORDINAL: core::cell::Cell<u32> = const { core::cell::Cell::new(0) };
    }
    let ordinal = ORDINAL.with(|cell| {
        let next = cell.get().saturating_add(1);
        cell.set(next);
        next
    });
    let case = std::thread::current()
        .name()
        .unwrap_or("unnamed")
        .to_owned();
    let disposition = match result {
        Ok(()) => "executed".to_owned(),
        Err(TransactionError::InstructionError(_, InstructionError::Custom(code))) => {
            format!("refused 0x{code:x}")
        }
        Err(_) => "refused".to_owned(),
    };
    let failure = result.as_ref().err().map(|error| format!("{error:?}"));
    let label = format!("direct: {case} #{ordinal} -- {disposition}");
    dclutch_program_test_evidence::record(&dclutch_program_test_evidence::TransactionEvidence {
        label: &label,
        signature,
        slot,
        error: failure.as_deref(),
        logs,
        compute_units_consumed,
        wire_bytes: Some(wire_bytes),
    })
    .expect("campaign evidence must be writable when the gauntlet asked for it");
}

/// The canonical budgeted transparent continuation packet, in wire bytes.
///
/// One signature and its count byte plus the serialized v0 message of the
/// FOUR-instruction canonical frame: SetComputeUnitLimit, RequestHeapFrame, the
/// Ed25519 precompile, and the Registry continuation carrying the
/// sealed-execution aliases. `submit_v0_observed` pins it whenever it
/// recognizes that exact frame, and `tests/hot_heap_frame_is_inert.rs` reads it
/// for the same packet. The derivation of the number is in
/// `submit_v0_observed`, beside the assertion that reads it.
pub const TRANSPARENT_CONTINUATION_WIRE_BYTES_V1: usize = 1_175;

/// Submit one canonical v0 transaction and retain exact success metadata.
pub async fn submit_v0_observed(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    addresses: Vec<Pubkey>,
    transaction_payer: Option<&Keypair>,
    signers: &[&Keypair],
) -> Result<SuccessfulExecution, RefusedExecution> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let transaction_payer = transaction_payer.unwrap_or(&context.payer);
    let message = VersionedMessage::V0(
        v0::Message::try_compile(
            &transaction_payer.pubkey(),
            instructions,
            &[AddressLookupTableAccount {
                key: LOOKUP_TABLE,
                addresses,
            }],
            blockhash,
        )
        .expect("canonical v0 message"),
    );
    let wire = 1_usize
        .checked_add(
            64_usize
                .checked_mul(signers.len() + 1)
                .expect("signature span"),
        )
        .and_then(|prefix| prefix.checked_add(message.serialize().len()))
        .expect("v0 wire width");
    assert!(
        wire <= 1_232,
        "canonical continuation packet overflow: {wire} bytes"
    );
    if instructions.len() == 4
        && instructions
            .first()
            .is_some_and(|instruction| instruction.program_id == compute_budget::ID)
        && instructions
            .get(1)
            .is_some_and(|instruction| instruction.program_id == compute_budget::ID)
        && instructions
            .get(2)
            .is_some_and(|instruction| instruction.program_id == ed25519_program::ID)
        && instructions
            .get(3)
            .is_some_and(|instruction| instruction.program_id == REGISTRY_PROGRAM_ID)
        && instructions
            .get(3)
            .is_some_and(has_canonical_sealed_execution_aliases)
    {
        // Compact native evidence references both maker keys from the exact
        // following Registry/Trading bytes instead of restating 64 bytes. That
        // pays for the mandatory 40-byte SetComputeUnitLimit addition and leaves
        // 28 bytes before the validated-artifact seal was introduced:
        // 1,228 + 40 - 64 = 1,204. The execution-only seal projection aliases
        // its six authenticated staging coordinates to the matching raw
        // coordinates, removing six lookup indexes, producing 1,198.
        //
        // 1,167 since 2026-09-02, and the 31 bytes are ONE key -- the System
        // program -- moving out of the static set and into the table: a static
        // key costs 32 bytes, a lookup index costs 1.
        //
        // 1,175 since 2026-09-03: the canonical frame carries its own
        // `RequestHeapFrame` now, at eight bytes -- a program-id index, an empty
        // account-index vector, and five bytes of data behind a length byte,
        // with no key of its own because the instruction above it is already a
        // ComputeBudget one. Fifty-seven bytes still spare against the ceiling.
        // See `direct_registry_instructions` for why the continuation stopped
        // fitting the protocol default heap.
        //
        // The number is a CONSTANT now rather than a literal here, because that
        // 31-byte step was applied to this pin and not to the sibling pin in
        // `tests/hot_heap_frame_is_inert.rs`, which measures this same packet
        // plus one appended instruction. It sat 31 bytes stale from `74e044cf3`
        // until 2026-09-03 and nothing said so -- the row it lives in was red
        // for an unrelated reason the whole time. Two callers, one author.
        //
        // `Pubkey::default()` and the System program id are the same 32 zero
        // bytes. The superseded derivation filtered `meta.pubkey != payer`, and
        // every caller here that has not chosen a payer yet passes
        // `Pubkey::default()` -- so it excluded the SYSTEM PROGRAM from the
        // table, by coincidence of encoding rather than by any rule. The
        // operator's builder refuses a default payer outright, so it has always
        // looked the System program up; this pin was measuring a packet the
        // protocol never emits, 31 bytes pessimistic. Both are one author now
        // and the harness measures the operator's wire.
        //
        // Looking it up is legal because only a TOP-LEVEL instruction's program
        // id must be a static key. `solana-message`'s `CompiledKeys` marks those
        // `is_invoked` and never drains them into a table; a program reached by
        // CPI carries no such rule, and the two cases below execute the whole
        // Direct Hot action with it looked up.
        assert_eq!(
            wire, TRANSPARENT_CONTINUATION_WIRE_BYTES_V1,
            "budgeted transparent continuation wire changed"
        );
    }
    let mut all_signers = vec![transaction_payer];
    all_signers.extend_from_slice(signers);
    let transaction = VersionedTransaction::try_new(message, &all_signers)
        .expect("complete canonical v0 signatures");
    let signature = transaction
        .signatures
        .first()
        .copied()
        .expect("a signed transaction carries at least the payer signature")
        .to_string();
    let slot = context
        .banks_client
        .get_root_slot()
        .await
        .expect("the bank reports a root slot");
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await?;
    let logs = processed
        .metadata
        .as_ref()
        .map(|metadata| metadata.log_messages.clone())
        .unwrap_or_default();
    record_campaign_transaction(
        &signature,
        slot,
        wire,
        &logs,
        &processed.result,
        processed
            .metadata
            .as_ref()
            .map(|metadata| metadata.compute_units_consumed),
    );
    if let Err(error) = processed.result {
        return Err(RefusedExecution {
            error: BanksClientError::TransactionError(error),
            logs,
            compute_units_consumed: processed
                .metadata
                .as_ref()
                .map(|metadata| metadata.compute_units_consumed)
                .unwrap_or_default(),
        });
    }
    Ok(processed
        .metadata
        .map_or_else(SuccessfulExecution::default, |metadata| {
            SuccessfulExecution {
                compute_units_consumed: metadata.compute_units_consumed,
                return_data: metadata
                    .return_data
                    .map(|returned| (returned.program_id, returned.data)),
                logs: metadata.log_messages,
            }
        }))
}

fn has_canonical_sealed_execution_aliases(instruction: &Instruction) -> bool {
    const REGISTRY_HOT_CHILD_START: usize = 6;
    // The FIFTH copy of the ABI's six pairs, and the second in this crate. Read
    // from the one declaration instead; see `alias_sealed_execution_metas`.
    const SEALED_ALIASES: [(usize, usize); 6] = SEALED_EXECUTION_FIXED_ALIASES_V3;
    let fixed = instruction
        .accounts
        .get(REGISTRY_HOT_CHILD_START..REGISTRY_HOT_CHILD_START + HOT_FIXED_ACCOUNT_COUNT_V3);
    let Some(fixed) = fixed else {
        return false;
    };
    SEALED_ALIASES.iter().all(|(raw, staging)| {
        fixed.get(*raw).map(|meta| meta.pubkey) == fixed.get(*staging).map(|meta| meta.pubkey)
    }) && fixed
        .iter()
        .map(|meta| meta.pubkey)
        .collect::<HashSet<_>>()
        .len()
        == HOT_FIXED_ACCOUNT_COUNT_V3 - SEALED_ALIASES.len()
}

/// Exact metadata retained from one successful canonical v0 execution.
#[derive(Default)]
pub struct SuccessfulExecution {
    /// Compute units the runtime charged.
    pub compute_units_consumed: u64,
    /// Final transaction return-data producer and bytes, when present.
    pub return_data: Option<(Pubkey, Vec<u8>)>,
    /// The program log, which a refusal already retained and a success did not.
    ///
    /// A CU figure says how much the transaction cost and nothing at all about
    /// WHICH ROUTES it ran, and on this route that difference is the whole
    /// question: the same instruction bytes invoke Custody once when the fee
    /// floors to zero and twice when it does not. The invocation count lives
    /// here, so a measurement can state the shape it measured rather than
    /// asserting it from the fixture's own arithmetic.
    pub logs: Vec<String>,
}

/// One refused execution together with the program log it reached.
///
/// A refusal test that only asserts `is_err()` cannot tell a refusal reached at
/// its intended depth from one that aborted before any of the CPIs it claims to
/// roll back ever ran.
pub struct RefusedExecution {
    pub error: BanksClientError,
    pub logs: Vec<String>,
    /// Compute consumed before the refusal, when the runtime reported any.
    ///
    /// A refusal has a price too, and for a slot-pin refusal it is the whole
    /// point: the pin is claimed to be reached early and cheaply, and a
    /// measurement that only asserted the discriminant could not tell a refusal
    /// raised at the Registry outer from one raised after most of a market
    /// action had already been paid for. Zero when the runtime produced no
    /// metadata, which is a transport failure rather than a program refusal.
    pub compute_units_consumed: u64,
}

impl From<BanksClientError> for RefusedExecution {
    fn from(error: BanksClientError) -> Self {
        Self {
            error,
            logs: Vec::new(),
            compute_units_consumed: 0,
        }
    }
}

impl core::fmt::Debug for RefusedExecution {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{:?}", self.error)
    }
}

impl RefusedExecution {
    pub fn invoked(&self, program: Pubkey) -> bool {
        let expected = format!("Program {program} invoke");
        self.logs.iter().any(|line| line.starts_with(&expected))
    }
}

pub fn program_test(artifacts: &Elves) -> ProgramTest {
    program_test_v2(artifacts, fixture_substrate())
}

/// The same substrate WITHOUT a forced compute budget.
///
/// `set_compute_max_units` makes solana-program-test install one fixed budget
/// and ignore the transaction's own ComputeBudget instructions -- including
/// `RequestHeapFrame`. A route that must be exercised WITH its heap grant
/// therefore cannot use `program_test`: the sysvar still shows the request, the
/// adapter authenticates it and lifts its ceiling, and the runtime maps the
/// default anyway, so the first scratch write lands outside the mapping and the
/// run dies on an access violation that looks like a program bug and is not.
/// That cost this lane an hour; it is a harness fact and it is named here now.
pub fn program_test_without_forced_budget(artifacts: &Elves) -> ProgramTest {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    let substrate = fixture_substrate();
    for (name, id, elf) in [
        (
            "dclutch_registry_sbf",
            REGISTRY_PROGRAM_ID,
            &artifacts.registry,
        ),
        (
            "dclutch_trading_sbf",
            TRADING_PROGRAM_ID,
            &artifacts.trading,
        ),
        ("dclutch_core_sbf", CORE_PROGRAM_ID, &artifacts.core),
        ("dclutch_claims_sbf", CLAIMS_PROGRAM_ID, &artifacts.claims),
        (
            "dclutch_custody_sbf",
            CUSTODY_PROGRAM_ID,
            &artifacts.custody,
        ),
    ] {
        add_program_v2(&mut test, name, id, elf, substrate);
    }
    test
}

pub fn program_test_v2(artifacts: &Elves, substrate: FixtureSubstrateV1) -> ProgramTest {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(COMPUTE_LIMIT);
    add_program_v2(
        &mut test,
        "dclutch_registry_sbf",
        REGISTRY_PROGRAM_ID,
        &artifacts.registry,
        substrate,
    );
    add_program_v2(
        &mut test,
        "dclutch_trading_sbf",
        TRADING_PROGRAM_ID,
        &artifacts.trading,
        substrate,
    );
    add_program_v2(
        &mut test,
        "dclutch_core_sbf",
        CORE_PROGRAM_ID,
        &artifacts.core,
        substrate,
    );
    add_program_v2(
        &mut test,
        "dclutch_claims_sbf",
        CLAIMS_PROGRAM_ID,
        &artifacts.claims,
        substrate,
    );
    add_program_v2(
        &mut test,
        "dclutch_custody_sbf",
        CUSTODY_PROGRAM_ID,
        &artifacts.custody,
        substrate,
    );
    test
}
