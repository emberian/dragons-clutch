//! Fixture plumbing shared by every campaign in this crate.
//!
//! # Why these moved out of a test binary
//!
//! Half of what is here encodes a **real on-chain format** by hand: a
//! Token-2022 Mint with its extension TLV, a Token-2022 token account, an
//! upgradeable `ProgramData` account, a Registry activation cache. A campaign
//! that keeps its own copy of one of those is the exact defect
//! `tools/seam-audit` exists to kill -- `SEAM_AUDIT_2026_08_29.md` closes on
//! *"a green suite is evidence about fixtures, not about seams"*, and its
//! worked example is a Token-2022 writer and its readers each fabricating the
//! mint bytes they expected. Two campaigns fabricating them separately is the
//! same defect with a second author, and it stays green on both sides while
//! they drift.
//!
//! So the fractional compaction campaign did not copy them. The three format
//! encoders live here, `tests/fractional_atomic.rs` was rewritten to delegate
//! to them, and a byte that is wrong is now wrong for every campaign at once --
//! which is the only arrangement under which a passing fixture is evidence
//! about anything. Verified by the delegation itself: the atomic campaign's
//! thirteen tests pass unchanged across the move, so the extraction is
//! byte-faithful rather than merely plausible.
//!
//! # And the plumbing came too
//!
//! An earlier version of this module shared only the three format encoders and
//! recorded the rest as debt: the atomic campaign kept its own `add_account`,
//! `add_upgradeable_program`, `finalized`, `activation_cache` and helpers,
//! identical to the ones below. That was a second author for a dozen functions,
//! introduced by the very lane that moved the first three out to avoid exactly
//! that -- so it was finished rather than left. Everything here now has one
//! author, and both campaigns' suites pass unchanged across the move (13/13
//! atomic, 5/5 compaction), which is what makes it an extraction.
//!
//! # What is parameterized, and what is not
//!
//! The program ids are arguments rather than constants, because the campaigns
//! genuinely differ there: an atomic campaign binds the Trading role to
//! `fractional-atomic-caller`, and a compaction campaign binds it to
//! `fractional-compaction-caller`, since the Fractional capability root is
//! derived under whichever program must `invoke_signed` for it. Nothing else
//! differs, and nothing else is parameterized -- an argument nobody varies is
//! a knob that lets one campaign quietly stop testing what the other does.

use solana_account::Account;
use solana_program::{hash::hash, pubkey::Pubkey, rent::Rent};
use solana_program_test::ProgramTest;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};

use dclutch_core_contract::ContentId;
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ActivatedExecutionReleaseSetV1,
    ArtifactActivationInputV1, ArtifactReleaseV1, ArtifactUpgradePolicyV1, DeploymentObservationV1,
    activate_execution_role_into_v1, initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1,
    ProgramIdentityV1,
};
use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;

use crate::narrow_fixture::NarrowRecordV2;

/// Exact Token-2022 token-account width.
pub const TOKEN_ACCOUNT_BYTES: usize = 165;

/// Offset at which a Token-2022 Mint's extension TLV begins.
///
/// A Mint carrying extensions is padded to the base *Account* width and then
/// tagged, so this is deliberately not the legacy 82-byte layout.
pub const MINT_TLV_START: usize = 166;

/// The Token-2022 program's address, as a `Pubkey`.
#[must_use]
pub fn token_program_id() -> Pubkey {
    Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID)
}

/// Wrap one program address as a Registry program identity.
///
/// # Panics
/// If the address is the unset pubkey, which no program has.
#[must_use]
pub fn identity(key: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(key.to_bytes()).expect("nonzero program identity")
}

/// The upgradeable loader's `ProgramData` address for one program.
#[must_use]
pub fn programdata_address(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

/// Copy `input` into `output` at `offset`, refusing a write past the end.
///
/// # Panics
/// If the span does not fit, which in a fixture means the layout moved.
pub fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    let end = offset.checked_add(input.len()).expect("fixture offset");
    output
        .get_mut(offset..end)
        .expect("fixture field")
        .copy_from_slice(input);
}

/// A `ProgramData` account holding one ELF with the upgrade authority cleared.
#[must_use]
pub fn immutable_programdata(elf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 45 + elf.len()];
    put(&mut bytes, 0, &3_u32.to_le_bytes());
    put(&mut bytes, 4, &0_u64.to_le_bytes());
    *bytes.get_mut(12).expect("ProgramData authority option") = 0;
    put(&mut bytes, 45, elf);
    bytes
}

/// Plant one rent-exempt account with the given owner and data.
pub fn add_account(test: &mut ProgramTest, key: Pubkey, owner: Pubkey, data: Vec<u8>) {
    test.add_account(
        key,
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    );
}

/// Plant one account at an exact lamport balance.
///
/// Separate from [`add_account`] because a campaign that observes a lamport
/// sweep must be able to open accounts at balances that are *not* the rent
/// minimum -- a fixture pinned to the minimum silently excuses any route that
/// recomputes the minimum instead of reading what is there.
pub fn add_account_with_lamports(
    test: &mut ProgramTest,
    key: Pubkey,
    owner: Pubkey,
    data: Vec<u8>,
    lamports: u64,
) {
    test.add_account(
        key,
        Account {
            lamports,
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    );
}

/// Add one upgradeable program to genesis together with its `ProgramData`.
pub fn add_upgradeable_program(
    test: &mut ProgramTest,
    name: &'static str,
    program: Pubkey,
    elf: &[u8],
) {
    test.add_upgradeable_program_to_genesis(name, &program);
    add_account(
        test,
        programdata_address(program),
        bpf_loader_upgradeable::ID,
        immutable_programdata(elf),
    );
}

/// One immutable artifact release for a program deployed at slot zero.
#[must_use]
pub fn release(program: Pubkey, semantic_seed: u8, elf: &[u8]) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        identity(program),
        identity(bpf_loader_upgradeable::ID),
        programdata_address(program).to_bytes(),
        ContentId::new([semantic_seed; 32]).expect("semantic release"),
        hash(elf).to_bytes(),
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("artifact release")
}

/// The content id one artifact release hashes to.
#[must_use]
pub fn artifact_id(value: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&value.to_bytes()).to_bytes()).expect("artifact ID")
}

/// Bind one release to its execution role.
#[must_use]
pub fn binding(value: ArtifactReleaseV1) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(value.program(), artifact_id(value))
}

/// The deployment observation a Registry activation checks a release against.
#[must_use]
pub fn activation_input(value: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
    let observation = DeploymentObservationV1::new(
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
    .expect("deployment observation");
    ArtifactActivationInputV1::new(artifact_id(value), value, observation)
}

/// The five execution roles a campaign's release set binds, with their ELFs.
///
/// `custody` is optional and the reason is a real refusal rather than
/// convenience: the replay-creation route refuses
/// `custody_program.key == program_id`, so any campaign that composes Custody
/// must bind a program distinct from Claims, while a campaign that never
/// reaches Custody may leave the role pointing at Claims.
pub struct ReleaseSetInputV1<'elf> {
    /// Core program address and ELF.
    pub core: (Pubkey, &'elf [u8]),
    /// Claims program address and ELF.
    pub claims: (Pubkey, &'elf [u8]),
    /// The Trading-role program: whichever test caller signs the root here.
    pub trading: (Pubkey, &'elf [u8]),
    /// Custody program and ELF, or `None` to point the role at Claims.
    pub custody: Option<(Pubkey, &'elf [u8])>,
}

/// Build one activated release set and its Registry activation cache bytes.
///
/// Returns the release-set identity and the exact cache account data.
#[must_use]
pub fn activation_cache(input: &ReleaseSetInputV1<'_>) -> ([u8; 32], Vec<u8>) {
    let core = release(input.core.0, 0x31, input.core.1);
    let claims = release(input.claims.0, 0x32, input.claims.1);
    let trading = release(input.trading.0, 0x33, input.trading.1);
    let custody = match input.custody {
        None => claims,
        Some((program, elf)) => release(program, 0x34, elf),
    };
    let release_set = ExecutionReleaseSetV1::new(
        binding(core),
        binding(claims),
        binding(trading),
        binding(claims),
        binding(custody),
    )
    .expect("release set");
    let release_set_id = hash(&release_set.to_bytes()).to_bytes();
    let content = ContentId::new(release_set_id).expect("release-set ID");
    let mut bytes = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut bytes, content).expect("initialize cache");
    for (role, value) in [
        (ExecutionRoleV1::Core, core),
        (ExecutionRoleV1::Claims, claims),
        (ExecutionRoleV1::Trading, trading),
        (ExecutionRoleV1::Resolution, claims),
        (ExecutionRoleV1::Custody, custody),
    ] {
        activate_execution_role_into_v1(
            &mut bytes,
            content,
            &release_set,
            role,
            &activation_input(value),
        )
        .expect("activate role");
    }
    ActivatedExecutionReleaseSetV1::decode(&bytes).expect("complete cache");
    (release_set_id, bytes)
}

/// Reproduce the shared fixture's finalized-record PDA derivation.
#[must_use]
pub fn finalized(owner: Pubkey, schema: [u8; 32], bytes: Vec<u8>) -> NarrowRecordV2 {
    let digest = hash(&bytes).to_bytes();
    let (raw, raw_bump) =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &owner);
    let (staging, staging_bump) =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &owner);
    NarrowRecordV2 {
        owner,
        schema,
        digest,
        raw,
        staging,
        raw_bump,
        staging_bump,
        bytes,
    }
}

/// Plant one finalized record as its raw/staging pair, the cursor left vacant.
pub fn add_finalized(test: &mut ProgramTest, record: &NarrowRecordV2) {
    add_account(test, record.raw, record.owner, record.bytes.clone());
    add_account(test, record.staging, system_program::ID, Vec::new());
}

/// Exact Token-2022 Mint with one controller holding every authority.
///
/// Carries `MintCloseAuthority` (3) and `PermissionedBurn` (28) naming that
/// controller, no freeze authority: the exact shape
/// `Token2022BehaviorProfileV2::read_mint` requires and refuses anything else
/// for.
#[must_use]
pub fn mint_bytes(controller: Pubkey, supply: u64, decimals: u8) -> Vec<u8> {
    let mut bytes = vec![0_u8; MINT_TLV_START];
    put(&mut bytes, 0, &1_u32.to_le_bytes());
    put(&mut bytes, 4, controller.as_ref());
    put(&mut bytes, 36, &supply.to_le_bytes());
    *bytes.get_mut(44).expect("Mint decimals") = decimals;
    *bytes.get_mut(45).expect("Mint initialized") = 1;
    *bytes.get_mut(165).expect("Mint account type") = 1;
    for extension in [3_u16, 28_u16] {
        bytes.extend_from_slice(&extension.to_le_bytes());
        bytes.extend_from_slice(&32_u16.to_le_bytes());
        bytes.extend_from_slice(controller.as_ref());
    }
    bytes
}

/// Base Token-2022 Mint with no mint or freeze authority.
///
/// The collateral Realm's `RequireAbsent` policies are why this Mint carries
/// neither authority; unlike the shard Mints it has no extensions.
#[must_use]
pub fn collateral_mint_bytes(supply: u64, decimals: u8) -> Vec<u8> {
    let mut bytes = vec![0_u8; 82];
    put(&mut bytes, 0, &0_u32.to_le_bytes());
    put(&mut bytes, 36, &supply.to_le_bytes());
    *bytes.get_mut(44).expect("decimals") = decimals;
    *bytes.get_mut(45).expect("initialized") = 1;
    put(&mut bytes, 46, &0_u32.to_le_bytes());
    bytes
}

/// Exact initialized Token-2022 token account.
#[must_use]
pub fn token_account_bytes_for(mint: Pubkey, owner: Pubkey, amount: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; TOKEN_ACCOUNT_BYTES];
    put(&mut bytes, 0, mint.as_ref());
    put(&mut bytes, 32, owner.as_ref());
    put(&mut bytes, 64, &amount.to_le_bytes());
    put(&mut bytes, 72, &0_u32.to_le_bytes());
    *bytes.get_mut(108).expect("state") = 1;
    put(&mut bytes, 109, &0_u32.to_le_bytes());
    put(&mut bytes, 129, &0_u32.to_le_bytes());
    bytes
}

/// Read a Token-2022 Mint's base supply out of planted or written bytes.
#[must_use]
pub fn mint_supply(data: &[u8]) -> u64 {
    u64::from_le_bytes(data[36..44].try_into().expect("Mint supply"))
}

/// Read a Token-2022 token account's amount.
#[must_use]
pub fn token_amount(data: &[u8]) -> u64 {
    u64::from_le_bytes(data[64..72].try_into().expect("token amount"))
}
