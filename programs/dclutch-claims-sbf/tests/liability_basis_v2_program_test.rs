//! Real-ELF ProgramTest evidence for the LiabilityBasisV2 (`DCLLBX02`) Claims
//! route, at the shape the current tree actually has.
//!
//! Two cases here are REFUSALS that used to be positive cases. Both refusals
//! are current protocol truth, and neither is a weakened assertion standing in
//! for an unfinished repair:
//!
//! - **`TerminalRedeem` is retired.** `3f7017a` moved Product V3 terminal
//!   settlement to `rational_terminal_v3`, and `LiabilityBasisActionV2::new`
//!   now refuses the tag unconditionally, as does
//!   `LiabilityBasisActionKindV2::decode` on the wire byte. The positive case
//!   is DELETED rather than adapted, and one executed refusal stands in its
//!   place. Modern redemption already has real-ELF coverage in
//!   `rational_representation_v2_program_test.rs`
//!   (`real_sbf_terminal_hostile_joins_and_late_child_failure_are_atomic`,
//!   `real_sbf_losing_terminal_burns_raw_shards_without_custody_payout`) and a
//!   census campaign in `tools/gauntlet/claims-rational-representation-v2`;
//!   this file does not duplicate it.
//!
//! - **The `Split` half of this route cannot execute.** `84b1426` made an
//!   external-source debit on the V1 Custody wire an outright refusal
//!   (`dclutch-custody-sbf`, `execute_transfer`: `CustodySbfError::Instruction`)
//!   so that a correct-looking balance delta cannot leave delegated spending
//!   authority behind; external debits belong on the `DCLCUDQ2` delegated V2
//!   wire. `liability_basis_v2` still composes `OperationV1::Transfer` with
//!   `CompartmentV1::External` as the source for a split, so real Custody
//!   refuses every `DCLLBX02` split. The campaign submits the canonical split
//!   and records that refusal instead of pretending the route deposits.
//!
//! What still executes is `Merge`, so the campaign drives the whole
//! Claims -> Custody -> token composition through it: two hostile finalized
//! basis substitutions, a late child refusal that must roll a completed Custody
//! transfer back, and two committing merges that unwind the aggregate to zero.
//! The aggregate therefore starts with supply already installed, because with
//! the split retired at the Custody boundary nothing in this route can mint it.
//!
//! Every expected value is computed from production constants, encoders and
//! kernel planners; none is read back from a run.

use std::{env, fs, path::PathBuf, vec::Vec};

use dclutch_claims_sbf::liability_basis_v2::{
    LIABILITY_BASIS_ACCOUNT_COUNT_V2, LIABILITY_BASIS_MARKET_SEED_V2,
    LIABILITY_BASIS_SCHEMA_RELEASE_ID_V2, LiabilityBasisActionInputV2, LiabilityBasisActionKindV2,
    LiabilityBasisActionV2, LiabilityBasisMarketInputV2, LiabilityBasisPositionInputV2,
    LiabilityBasisSbfErrorV2, TERMINAL_COORDINATE_SCHEMA_RELEASE_ID_V2,
    encode_liability_basis_market_v2, encode_liability_basis_position_v2,
    encode_terminal_coordinate_v2,
};
use dclutch_claims_sbf::protocol_position_v2::{
    PROTOCOL_POSITION_ADMISSION_BYTES_V2, PROTOCOL_POSITION_ADMISSION_SEED_V2,
    PROTOCOL_POSITION_ADMIT_ACCOUNT_COUNT_V2, ProtocolPositionActionV2,
    ProtocolPositionAdmissionV2, ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2,
    ProtocolPositionRequestV2, ProtocolPositionSeedsV2,
};
use dclutch_claims_svm::{ClaimsAggregateSeedsV1, ClaimsPositionSeedsV1};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1,
    CustodyReplayV1, CustodyRequestV1, CustodyVaultSeedsV1, OperationV1,
};
use dclutch_economic_slice_kernel::{
    MARKET_HEADER_BYTES, Phase as EconomicPhase, SCALAR_BYTES, initialize_market,
};
use dclutch_liability_basis_v2_kernel::product_claims::{
    AdmittedBasisV2, CAPPED_RAMP_BASIS_BYTES_V2, CappedRampBasisInputV2, ClaimsCandidateV2,
    ContentIdV2, LINKED_CAPPED_RAMP_BASIS_BYTES_V2, encode_capped_ramp_basis_v2,
    encode_linked_basis_record_v2,
};
use dclutch_market_core_codec::{
    CoreState, Identity, MarketCoreStateSeedsV2, MarketIdentity, Phase as CorePhase, Readiness,
};
use dclutch_product_contract::{
    ContentId as ProductContentId,
    capacity::CapacityProfileId,
    product::{InstanceV1, InstanceV1Input, PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1},
};
use dclutch_program_test_evidence::TransactionEvidence;
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, CallerAuthoritySeedsV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1,
    ExecutionRoleV1, ProgramIdentityV1,
};
use dclutch_token_svm::{LEGACY_TOKEN_PROGRAM_ID, PRODUCTION_ADAPTER_RELEASES, TokenAccount};
use solana_account::Account;
use solana_address_lookup_table_interface::instruction::{
    create_lookup_table, extend_lookup_table,
};
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    clock::Clock,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_option::COption;
use solana_program_pack::Pack;
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::versioned::VersionedTransaction;
use spl_token_interface::state::{Account as SplAccount, AccountState, Mint as SplMint};

const CLAIMS_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xb1; 32]);
const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xb2; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xb3; 32]);
const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xb4; 32]);
const TEST_CALLER_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xb5; 32]);
const GENERATION: u64 = 23;
const SCALE: u64 = 10;
const CLAIM_COUNT: u32 = 2;
const BASIS_SEMANTIC_ID_DOMAIN_V2: &[u8] = b"dclutch/lbv2/semantic-id/v2";
const CANDIDATE_DIGEST_DOMAIN_V2: [u8; 27] = *b"dclutch/lbv2/candidate/v2\0\0";
/// Total collateral the fixture Mint has issued, split between the owner's
/// external account and the Hoard vault.
const MINT_SUPPLY: u64 = 100;
/// Installed aggregate supply and Position balance, one entry per claim.
///
/// `AdmittedBasisV2`'s solvency rule is `max(supply) * SCALE <= hoard`, so
/// three complete sets at `SCALE` ten pin [`INITIAL_HOARD`] at exactly the
/// maximum pre-resolution liability: the prestate is fully collateralised and
/// not over-collateralised.
const INITIAL_CLAIMS: [u64; CLAIM_COUNT as usize] = [3, 3];
/// Hoard-principal balance backing [`INITIAL_CLAIMS`]: `3 * SCALE`.
const INITIAL_HOARD: u64 = INITIAL_CLAIMS[0] * SCALE;

struct Artifacts {
    claims: Vec<u8>,
    custody: Vec<u8>,
    registry: Vec<u8>,
    core: Vec<u8>,
    caller: Vec<u8>,
}

struct Fixture {
    owner: Keypair,
    release_set: [u8; 32],
    realm_id: [u8; 32],
    context_id: [u8; 32],
    product_id: [u8; 32],
    semantic_basis_id: [u8; 32],
    embedded_basis: Vec<u8>,
    market_input: LiabilityBasisMarketInputV2,
    position_input: LiabilityBasisPositionInputV2,
    market: Pubkey,
    position: Pubkey,
    linked_basis_raw: Pubkey,
    linked_basis_staging: Pubkey,
    other_product_basis_raw: Pubkey,
    other_product_basis_staging: Pubkey,
    changed_payoff_basis_raw: Pubkey,
    changed_payoff_basis_staging: Pubkey,
    product_raw: Pubkey,
    product_staging: Pubkey,
    core_market: Pubkey,
    activation_cache: Pubkey,
    claims_programdata: Pubkey,
    custody_programdata: Pubkey,
    core_programdata: Pubkey,
    realm: Pubkey,
    realm_staging: Pubkey,
    replay: Pubkey,
    mint: Pubkey,
    external: Pubkey,
    hoard: Pubkey,
    custody_authority: Pubkey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Snapshot {
    market: Account,
    position: Account,
    replay: Account,
    external: Account,
    hoard: Account,
}

#[derive(Clone, Copy)]
struct StateModel {
    market_revision: u64,
    position_revision: u64,
    custody_revision: u64,
    supplies: [u64; 2],
    balances: [u64; 2],
    hoard: u64,
}

struct BuiltAction {
    direct: Instruction,
    wrapper: Instruction,
    request: CustodyRequestV1,
    after: StateModel,
}

fn artifacts() -> Artifacts {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    let read = |name: &str| {
        let path = directory.join(name);
        assert!(path.is_file(), "missing real ELF: {}", path.display());
        fs::read(path).expect("read real ELF")
    };
    Artifacts {
        claims: read("dclutch_claims_sbf.so"),
        custody: read("dclutch_custody_sbf.so"),
        registry: read("dclutch_registry_sbf.so"),
        core: read("dclutch_core_sbf.so"),
        caller: read("dclutch_claims_liability_basis_test_caller_sbf.so"),
    }
}

fn identity(key: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(key.to_bytes()).expect("nonzero program identity")
}

fn semantic_identity(bytes: [u8; 32]) -> Identity {
    Identity::new(bytes).expect("nonzero semantic identity")
}

fn product_id(bytes: [u8; 32]) -> ProductContentId {
    ProductContentId::new(bytes).expect("nonzero Product identity")
}

fn programdata_address(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn immutable_programdata(elf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 45 + elf.len()];
    put(&mut bytes, 0, &3_u32.to_le_bytes());
    put(&mut bytes, 4, &0_u64.to_le_bytes());
    *bytes.get_mut(12).expect("ProgramData authority option") = 0;
    put(&mut bytes, 45, elf);
    bytes
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    let end = offset.checked_add(input.len()).expect("fixture offset");
    output
        .get_mut(offset..end)
        .expect("fixture field")
        .copy_from_slice(input);
}

fn add_account(test: &mut ProgramTest, key: Pubkey, owner: Pubkey, data: Vec<u8>) {
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

fn add_upgradeable_program(
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

fn release(program: Pubkey, semantic_seed: u8, elf: &[u8]) -> ArtifactReleaseV1 {
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

fn artifact_id(release: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes()).expect("artifact ID")
}

fn binding(release: ArtifactReleaseV1) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(release.program(), artifact_id(release))
}

fn activation_input(release: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
    let observation = DeploymentObservationV1::new(
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
    .expect("deployment observation");
    ArtifactActivationInputV1::new(artifact_id(release), release, observation)
}

fn activation_cache(artifacts: &Artifacts) -> ([u8; 32], Vec<u8>) {
    let core = release(CORE_PROGRAM_ID, 0x51, &artifacts.core);
    let claims = release(CLAIMS_PROGRAM_ID, 0x52, &artifacts.claims);
    let custody = release(CUSTODY_PROGRAM_ID, 0x53, &artifacts.custody);
    let trading = release(TEST_CALLER_PROGRAM_ID, 0x54, &artifacts.caller);
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
    for (role, artifact) in [
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
            &activation_input(artifact),
        )
        .expect("activate role");
    }
    ActivatedExecutionReleaseSetV1::decode(&bytes).expect("complete cache");
    (release_set_id, bytes)
}

fn finalized_record_keys(schema: [u8; 32], digest: [u8; 32]) -> (Pubkey, Pubkey) {
    let raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, schema.as_slice(), digest.as_slice()],
        &CORE_PROGRAM_ID,
    )
    .0;
    let staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            schema.as_slice(),
            digest.as_slice(),
        ],
        &CORE_PROGRAM_ID,
    )
    .0;
    (raw, staging)
}

fn add_finalized_record(
    test: &mut ProgramTest,
    schema: [u8; 32],
    bytes: &[u8],
) -> (Pubkey, Pubkey, [u8; 32]) {
    let digest = hash(bytes).to_bytes();
    let (raw, staging) = finalized_record_keys(schema, digest);
    add_account(test, raw, CORE_PROGRAM_ID, bytes.to_vec());
    add_account(test, staging, system_program::ID, Vec::new());
    (raw, staging, digest)
}

fn add_registry_finalized_realm(
    test: &mut ProgramTest,
    bytes: &[u8],
) -> (Pubkey, Pubkey, [u8; 32]) {
    let digest = hash(bytes).to_bytes();
    let raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &REALM_SCHEMA_RELEASE_ID_V1, &digest],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    let staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            &digest,
        ],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    add_account(test, raw, REGISTRY_PROGRAM_ID, bytes.to_vec());
    add_account(test, staging, system_program::ID, Vec::new());
    (raw, staging, digest)
}

struct BasisArtifacts {
    semantic_id: [u8; 32],
    product_id: [u8; 32],
    product_bytes: Vec<u8>,
    embedded: Vec<u8>,
    linked: Vec<u8>,
    other_product_linked: Vec<u8>,
    changed_payoff_linked: Vec<u8>,
}

fn basis_artifacts() -> BasisArtifacts {
    let placeholder = ContentIdV2::new([1; 32]).expect("placeholder Product ID");
    let mut embedded = [0_u8; CAPPED_RAMP_BASIS_BYTES_V2];
    encode_capped_ramp_basis_v2(
        CappedRampBasisInputV2 {
            product_instance_id: placeholder,
            knot_denominator: 1,
            left_numerator: 0,
            right_numerator: 1,
            scale: SCALE,
        },
        &mut embedded,
    )
    .expect("ramp basis");
    let semantic_id = hashv(&[
        BASIS_SEMANTIC_ID_DOMAIN_V2,
        embedded.get(..32).expect("semantic prefix"),
        embedded.get(64..).expect("semantic suffix"),
    ])
    .to_bytes();
    let capacity = CapacityProfileId::new(product_id([0x61; 32]));
    let instance = InstanceV1::new(InstanceV1Input {
        terms_id: product_id([0x62; 32]),
        occurrence_id: product_id([0x63; 32]),
        claim_basis_id: product_id(semantic_id),
        result_domain_id: product_id([0x64; 32]),
        capacity_profile_id: capacity,
        partition_cell_count: CLAIM_COUNT,
    })
    .expect("Product instance");
    let product_bytes = instance.to_bytes().to_vec();
    let product_digest = hash(&product_bytes).to_bytes();
    let product_content = ContentIdV2::new(product_digest).expect("Product digest");
    encode_capped_ramp_basis_v2(
        CappedRampBasisInputV2 {
            product_instance_id: product_content,
            knot_denominator: 1,
            left_numerator: 0,
            right_numerator: 1,
            scale: SCALE,
        },
        &mut embedded,
    )
    .expect("Product-linked ramp basis");
    assert_eq!(
        hashv(&[
            BASIS_SEMANTIC_ID_DOMAIN_V2,
            embedded.get(..32).expect("semantic prefix"),
            embedded.get(64..).expect("semantic suffix"),
        ])
        .to_bytes(),
        semantic_id,
        "Product linkage is outside semantic basis identity"
    );
    let semantic_content = ContentIdV2::new(semantic_id).expect("semantic basis ID");
    let mut linked = [0_u8; LINKED_CAPPED_RAMP_BASIS_BYTES_V2];
    encode_linked_basis_record_v2(product_content, semantic_content, &embedded, &mut linked)
        .expect("final Product link");

    let other_instance = InstanceV1::new(InstanceV1Input {
        occurrence_id: product_id([0x65; 32]),
        ..InstanceV1Input {
            terms_id: product_id([0x62; 32]),
            occurrence_id: product_id([0x63; 32]),
            claim_basis_id: product_id(semantic_id),
            result_domain_id: product_id([0x64; 32]),
            capacity_profile_id: capacity,
            partition_cell_count: CLAIM_COUNT,
        }
    })
    .expect("other Product instance");
    let other_product_digest = hash(&other_instance.to_bytes()).to_bytes();
    let other_product = ContentIdV2::new(other_product_digest).expect("other Product ID");
    let mut other_embedded = [0_u8; CAPPED_RAMP_BASIS_BYTES_V2];
    encode_capped_ramp_basis_v2(
        CappedRampBasisInputV2 {
            product_instance_id: other_product,
            knot_denominator: 1,
            left_numerator: 0,
            right_numerator: 1,
            scale: SCALE,
        },
        &mut other_embedded,
    )
    .expect("same semantics, other Product");
    assert_eq!(
        hashv(&[
            BASIS_SEMANTIC_ID_DOMAIN_V2,
            other_embedded.get(..32).expect("semantic prefix"),
            other_embedded.get(64..).expect("semantic suffix"),
        ])
        .to_bytes(),
        semantic_id
    );
    let mut other_product_linked = [0_u8; LINKED_CAPPED_RAMP_BASIS_BYTES_V2];
    encode_linked_basis_record_v2(
        other_product,
        semantic_content,
        &other_embedded,
        &mut other_product_linked,
    )
    .expect("other Product finalized link");

    let mut changed_payoff = [0_u8; CAPPED_RAMP_BASIS_BYTES_V2];
    encode_capped_ramp_basis_v2(
        CappedRampBasisInputV2 {
            product_instance_id: product_content,
            knot_denominator: 1,
            left_numerator: 0,
            right_numerator: 2,
            scale: SCALE,
        },
        &mut changed_payoff,
    )
    .expect("changed payoff basis");
    assert_ne!(
        hashv(&[
            BASIS_SEMANTIC_ID_DOMAIN_V2,
            changed_payoff.get(..32).expect("semantic prefix"),
            changed_payoff.get(64..).expect("semantic suffix"),
        ])
        .to_bytes(),
        semantic_id
    );
    let mut changed_payoff_linked = [0_u8; LINKED_CAPPED_RAMP_BASIS_BYTES_V2];
    encode_linked_basis_record_v2(
        product_content,
        semantic_content,
        &changed_payoff,
        &mut changed_payoff_linked,
    )
    .expect("hostile stale semantic link");

    BasisArtifacts {
        semantic_id,
        product_id: product_digest,
        product_bytes,
        embedded: embedded.to_vec(),
        linked: linked.to_vec(),
        other_product_linked: other_product_linked.to_vec(),
        changed_payoff_linked: changed_payoff_linked.to_vec(),
    }
}

fn mint_data(supply: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; SplMint::LEN];
    SplMint::pack(
        SplMint {
            mint_authority: COption::None,
            supply,
            decimals: 0,
            is_initialized: true,
            freeze_authority: COption::None,
        },
        &mut bytes,
    )
    .expect("pack Mint");
    bytes
}

fn token_account_data(
    mint: Pubkey,
    owner: Pubkey,
    amount: u64,
    delegate: COption<Pubkey>,
    delegated_amount: u64,
) -> Vec<u8> {
    let mut bytes = vec![0_u8; SplAccount::LEN];
    SplAccount::pack(
        SplAccount {
            mint,
            owner,
            amount,
            delegate,
            state: AccountState::Initialized,
            is_native: COption::None,
            delegated_amount,
            close_authority: COption::None,
        },
        &mut bytes,
    )
    .expect("pack token account");
    bytes
}

fn fixture(terminal: bool) -> (ProgramTest, Fixture, StateModel) {
    let artifacts = artifacts();
    let basis = basis_artifacts();
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    for (name, id, elf) in [
        (
            "dclutch_claims_sbf",
            CLAIMS_PROGRAM_ID,
            artifacts.claims.as_slice(),
        ),
        (
            "dclutch_custody_sbf",
            CUSTODY_PROGRAM_ID,
            artifacts.custody.as_slice(),
        ),
        (
            "dclutch_registry_sbf",
            REGISTRY_PROGRAM_ID,
            artifacts.registry.as_slice(),
        ),
        (
            "dclutch_core_sbf",
            CORE_PROGRAM_ID,
            artifacts.core.as_slice(),
        ),
        (
            "dclutch_claims_liability_basis_test_caller_sbf",
            TEST_CALLER_PROGRAM_ID,
            artifacts.caller.as_slice(),
        ),
    ] {
        add_upgradeable_program(&mut test, name, id, elf);
    }

    let owner = Keypair::new_from_array(if terminal { [0xc2; 32] } else { [0xc1; 32] });
    add_account(&mut test, owner.pubkey(), system_program::ID, Vec::new());
    let (release_set, cache_data) = activation_cache(&artifacts);
    let activation_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    add_account(&mut test, activation_cache, REGISTRY_PROGRAM_ID, cache_data);

    let mint = Pubkey::new_from_array(if terminal { [0xc4; 32] } else { [0xc3; 32] });
    let token_program = Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID);
    let adapter = PRODUCTION_ADAPTER_RELEASES
        .first()
        .copied()
        .expect("legacy production adapter");
    let realm_value = RealmV1::new(RealmV1Input {
        token_program: token_program.to_bytes(),
        collateral_mint: mint.to_bytes(),
        collateral_adapter_release_id: hash(&adapter.to_bytes()).to_bytes(),
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .expect("Realm");
    let realm_data = realm_value.to_bytes().to_vec();
    let (realm, realm_staging, realm_id) = add_registry_finalized_realm(&mut test, &realm_data);

    let (product_raw, product_staging, product_digest) = add_finalized_record(
        &mut test,
        PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1,
        &basis.product_bytes,
    );
    assert_eq!(product_digest, basis.product_id);

    // No live LiabilityBasisV2 action reads a terminal coordinate any more --
    // the tag that did is retired. The record is still finalized so that a
    // terminal Core state can carry a real `terminal_receipt` digest rather
    // than an invented one; nothing else consumes it here.
    let coordinate = encode_terminal_coordinate_v2(1, 2).expect("terminal coordinate");
    let (_, _, coordinate_digest) = add_finalized_record(
        &mut test,
        TERMINAL_COORDINATE_SCHEMA_RELEASE_ID_V2,
        &coordinate,
    );
    let mut core_identity = MarketIdentity {
        market_id: semantic_identity([1; 32]),
        realm_id: semantic_identity(realm_id),
        product_record: semantic_identity(product_digest),
        product_id: semantic_identity(basis.product_id),
        resolution_policy: semantic_identity([0x71; 32]),
        capability_manifest: semantic_identity([0x72; 32]),
        selected_release_set: semantic_identity(release_set),
        registry_program: semantic_identity(REGISTRY_PROGRAM_ID.to_bytes()),
        generation: GENERATION,
    };
    let core_market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(core_identity).as_slices(),
        &CORE_PROGRAM_ID,
    )
    .0;
    core_identity.market_id = semantic_identity(core_market.to_bytes());
    let core = CoreState {
        phase: if terminal {
            CorePhase::Terminal
        } else {
            CorePhase::Open
        },
        readiness: Readiness::Consumed,
        terminal_winner: 0,
        identity: core_identity,
        outstanding_capabilities: 1,
        rent_beneficiary: semantic_identity(owner.pubkey().to_bytes()),
        terminal_receipt: terminal.then(|| semantic_identity(coordinate_digest)),
    };
    add_account(
        &mut test,
        core_market,
        CORE_PROGRAM_ID,
        core.encode().expect("Core state").to_vec(),
    );

    let (linked_basis_raw, linked_basis_staging, _) = add_finalized_record(
        &mut test,
        LIABILITY_BASIS_SCHEMA_RELEASE_ID_V2,
        &basis.linked,
    );
    let (other_product_basis_raw, other_product_basis_staging, _) = add_finalized_record(
        &mut test,
        LIABILITY_BASIS_SCHEMA_RELEASE_ID_V2,
        &basis.other_product_linked,
    );
    let (changed_payoff_basis_raw, changed_payoff_basis_staging, _) = add_finalized_record(
        &mut test,
        LIABILITY_BASIS_SCHEMA_RELEASE_ID_V2,
        &basis.changed_payoff_linked,
    );

    let market = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, core_market.as_ref()],
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let position_seeds = ProtocolPositionSeedsV2::new(market.to_bytes(), owner.pubkey().to_bytes())
        .expect("LBV2 Position seeds");
    let position = Pubkey::find_program_address(&position_seeds.as_slices(), &CLAIMS_PROGRAM_ID).0;
    let context_id = if terminal { [0x82; 32] } else { [0x81; 32] };
    let market_input = LiabilityBasisMarketInputV2 {
        revision: 0,
        logical_market: core_market.to_bytes(),
        release_set,
        registry_program: REGISTRY_PROGRAM_ID.to_bytes(),
        product_instance_id: basis.product_id,
        basis_id: basis.semantic_id,
        realm_id,
        custody_context: context_id,
        generation: GENERATION,
    };
    let position_input = LiabilityBasisPositionInputV2 {
        revision: 0,
        market_account: market.to_bytes(),
        owner: owner.pubkey().to_bytes(),
        basis_id: basis.semantic_id,
    };
    // Both worlds start with supply and a matching Hoard balance. Splitting is
    // how this route used to mint supply, and real Custody refuses every split
    // it composes (see the module doc), so an installed prestate is the only
    // honest way to reach a merge at all.
    let initial_claims = INITIAL_CLAIMS;
    add_account(
        &mut test,
        market,
        CLAIMS_PROGRAM_ID,
        encode_liability_basis_market_v2(market_input, &initial_claims).expect("Claims aggregate"),
    );
    add_account(
        &mut test,
        position,
        CLAIMS_PROGRAM_ID,
        encode_liability_basis_position_v2(position_input, &initial_claims)
            .expect("Claims Position"),
    );

    let base_request = CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CallerRoleV1::Claims,
        source_compartment: CompartmentV1::External,
        destination_compartment: CompartmentV1::HoardPrincipal,
        release_set,
        market: core_market.to_bytes(),
        realm: realm_id,
        context: context_id,
        caller_program: CLAIMS_PROGRAM_ID.to_bytes(),
        semantic: ContextV1 {
            candidate: [1; 32],
            source_owner: owner.pubkey().to_bytes(),
            destination_owner: [0; 32],
            order: [0; 32],
            parent_request_digest: [2; 32],
            order_nonce: 1,
            generation: GENERATION,
            page_index: 0,
            execution_index: 0,
            transfer_index: 0,
        },
        source: [3; 32],
        destination: [4; 32],
        source_vault_context: [0; 32],
        destination_vault_context: context_id,
        mint: mint.to_bytes(),
        token_program: token_program.to_bytes(),
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: 1,
        resulting_revision: 2,
        amount: 1,
        rent_lamports: 0,
    };
    let custody_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::from_request(base_request).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::from_request(base_request).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    // The Hoard Vault is namespaced by the aggregate's `custody_context`, the
    // same coordinate the replay above uses. This fixture named the Core Market
    // address here while telling the aggregate the context was `context_id` --
    // two different namespaces asserted about one Market, which is exactly the
    // shape the founding routes produce and no payout route could follow.
    let hoard = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            core_market.to_bytes(),
            release_set,
            context_id,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let external = Pubkey::new_from_array(if terminal { [0xc6; 32] } else { [0xc5; 32] });
    let initial_hoard = INITIAL_HOARD;
    let initial_external = MINT_SUPPLY - initial_hoard;
    add_account(&mut test, mint, token_program, mint_data(MINT_SUPPLY));
    add_account(
        &mut test,
        external,
        token_program,
        token_account_data(
            mint,
            owner.pubkey(),
            initial_external,
            COption::Some(custody_authority),
            initial_external,
        ),
    );
    add_account(
        &mut test,
        hoard,
        token_program,
        token_account_data(mint, custody_authority, initial_hoard, COption::None, 0),
    );
    let replay_state = CustodyReplayV1 {
        caller_role: CallerRoleV1::Claims,
        release_set,
        market: core_market.to_bytes(),
        realm: realm_id,
        context: context_id,
        caller_program: CLAIMS_PROGRAM_ID.to_bytes(),
        rent_refund: owner.pubkey().to_bytes(),
        open_vault_count: 1,
        next_revision: 1,
        generation: GENERATION,
        last_request_digest: [0x91; 32],
        last_poststate_commitment: [0x92; 32],
    };
    add_account(
        &mut test,
        replay,
        CUSTODY_PROGRAM_ID,
        replay_state.to_bytes().expect("Custody replay").to_vec(),
    );

    let state = StateModel {
        market_revision: 0,
        position_revision: 0,
        custody_revision: 1,
        supplies: initial_claims,
        balances: initial_claims,
        hoard: initial_hoard,
    };
    (
        test,
        Fixture {
            owner,
            release_set,
            realm_id,
            context_id,
            product_id: basis.product_id,
            semantic_basis_id: basis.semantic_id,
            embedded_basis: basis.embedded,
            market_input,
            position_input,
            market,
            position,
            linked_basis_raw,
            linked_basis_staging,
            other_product_basis_raw,
            other_product_basis_staging,
            changed_payoff_basis_raw,
            changed_payoff_basis_staging,
            product_raw,
            product_staging,
            core_market,
            activation_cache,
            claims_programdata: programdata_address(CLAIMS_PROGRAM_ID),
            custody_programdata: programdata_address(CUSTODY_PROGRAM_ID),
            core_programdata: programdata_address(CORE_PROGRAM_ID),
            realm,
            realm_staging,
            replay,
            mint,
            external,
            hoard,
            custody_authority,
        },
        state,
    )
}

fn admitted_basis(fixture: &Fixture) -> AdmittedBasisV2 {
    let basis_id = ContentIdV2::new(fixture.semantic_basis_id).expect("basis ID");
    AdmittedBasisV2::admit(
        &fixture.embedded_basis,
        basis_id,
        basis_id,
        ContentIdV2::new(fixture.product_id).expect("Product ID"),
    )
    .expect("admitted basis")
}

fn plan(
    fixture: &Fixture,
    state: StateModel,
    kind: LiabilityBasisActionKindV2,
    quantity: u64,
) -> (ClaimsCandidateV2, [u64; 2], [u64; 2]) {
    let basis = admitted_basis(fixture);
    let mut supplies = [0; 2];
    let mut balances = [0; 2];
    let planned = match kind {
        LiabilityBasisActionKindV2::Split => Some(basis.plan_split_into(
            &state.supplies,
            &state.balances,
            quantity,
            state.hoard,
            &mut supplies,
            &mut balances,
        )),
        LiabilityBasisActionKindV2::Merge => Some(basis.plan_merge_into(
            &state.supplies,
            &state.balances,
            quantity,
            state.hoard,
            &mut supplies,
            &mut balances,
        )),
        // The kernel still exposes `plan_terminal_redeem_into`, but no
        // LiabilityBasisV2 action can reach it: `LiabilityBasisActionV2::new`
        // refuses the tag. Planning one here would be a fixture inventing a
        // transition the program will not perform.
        LiabilityBasisActionKindV2::TerminalRedeem => None,
    };
    let candidate = planned
        .expect("TerminalRedeem is retired; encode it with retired_terminal_redeem_data")
        .expect("pure Claims candidate");
    (candidate, supplies, balances)
}

fn build_action(
    fixture: &Fixture,
    state: StateModel,
    kind: LiabilityBasisActionKindV2,
    quantity: u64,
    basis_raw: Pubkey,
    basis_staging: Pubkey,
) -> BuiltAction {
    let (candidate, supplies, balances) = plan(fixture, state, kind, quantity);
    // Production requires a canonical zero claim index for both live kinds and
    // refuses anything else at construction; the field exists only for the
    // retired terminal tag.
    let claim_index = 0;
    let action = LiabilityBasisActionV2::new(LiabilityBasisActionInputV2 {
        kind,
        custody_present: true,
        expected_market_revision: state.market_revision,
        expected_position_revision: state.position_revision,
        quantity,
        claim_index,
        expected_custody_revision: state.custody_revision,
        request_nonce: state.market_revision + 41,
    })
    .expect("Claims action");
    let action_bytes = action.to_bytes();
    let mut market_input = fixture.market_input;
    market_input.revision = state.market_revision + 1;
    let mut position_input = fixture.position_input;
    position_input.revision = state.position_revision + 1;
    let market_candidate =
        encode_liability_basis_market_v2(market_input, &supplies).expect("market candidate");
    let position_candidate =
        encode_liability_basis_position_v2(position_input, &balances).expect("position candidate");
    let candidate_digest = hashv(&[
        &CANDIDATE_DIGEST_DOMAIN_V2,
        &market_candidate,
        &position_candidate,
    ])
    .to_bytes();
    let split = kind == LiabilityBasisActionKindV2::Split;
    let source = if split {
        fixture.external
    } else {
        fixture.hoard
    };
    let destination = if split {
        fixture.hoard
    } else {
        fixture.external
    };
    let amount = candidate.collateral_in() + candidate.collateral_out();
    let request = CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CallerRoleV1::Claims,
        source_compartment: if split {
            CompartmentV1::External
        } else {
            CompartmentV1::HoardPrincipal
        },
        destination_compartment: if split {
            CompartmentV1::HoardPrincipal
        } else {
            CompartmentV1::External
        },
        release_set: fixture.release_set,
        market: fixture.core_market.to_bytes(),
        realm: fixture.realm_id,
        context: fixture.context_id,
        caller_program: CLAIMS_PROGRAM_ID.to_bytes(),
        semantic: ContextV1 {
            candidate: candidate_digest,
            source_owner: if split {
                fixture.owner.pubkey().to_bytes()
            } else {
                [0; 32]
            },
            destination_owner: if split {
                [0; 32]
            } else {
                fixture.owner.pubkey().to_bytes()
            },
            order: [0; 32],
            parent_request_digest: hash(&action_bytes).to_bytes(),
            order_nonce: state.market_revision + 41,
            generation: GENERATION,
            page_index: 0,
            execution_index: 0,
            transfer_index: 0,
        },
        source: source.to_bytes(),
        destination: destination.to_bytes(),
        // The HoardPrincipal side is namespaced by the aggregate's
        // `custody_context`, which this fixture already uses for the replay and
        // the caller PDA below. Naming the Core Market address here made this
        // one request assert two different namespaces for one Market.
        source_vault_context: if split { [0; 32] } else { fixture.context_id },
        destination_vault_context: if split { fixture.context_id } else { [0; 32] },
        mint: fixture.mint.to_bytes(),
        token_program: LEGACY_TOKEN_PROGRAM_ID,
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: state.custody_revision,
        resulting_revision: state.custody_revision + 1,
        amount,
        rent_lamports: 0,
    };
    let request_bytes = request.to_bytes().expect("Custody request");
    let caller_authority = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::new(
            ContentId::new(fixture.release_set).expect("release set"),
            fixture.core_market.to_bytes(),
            ExecutionRoleV1::Claims,
            fixture.context_id,
            hash(&request_bytes).to_bytes(),
        )
        .expect("Claims caller seeds")
        .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    // `authenticate_open_core` requires both terminal-coordinate slots to be
    // the Core program's own address for Split and Merge -- the only two kinds
    // that can be built at all. There is no longer a shape in which they carry
    // a record.
    let coordinate = CORE_PROGRAM_ID;
    let coordinate_staging = CORE_PROGRAM_ID;
    let metas = vec![
        AccountMeta::new_readonly(fixture.owner.pubkey(), true),
        AccountMeta::new(fixture.market, false),
        AccountMeta::new(fixture.position, false),
        AccountMeta::new_readonly(basis_raw, false),
        AccountMeta::new_readonly(basis_staging, false),
        AccountMeta::new_readonly(fixture.product_raw, false),
        AccountMeta::new_readonly(fixture.product_staging, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(fixture.core_market, false),
        AccountMeta::new_readonly(coordinate, false),
        AccountMeta::new_readonly(coordinate_staging, false),
        AccountMeta::new_readonly(fixture.activation_cache, false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.claims_programdata, false),
        AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.custody_programdata, false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.core_programdata, false),
        AccountMeta::new_readonly(caller_authority, false),
        AccountMeta::new_readonly(fixture.realm, false),
        AccountMeta::new_readonly(fixture.realm_staging, false),
        AccountMeta::new(fixture.replay, false),
        AccountMeta::new_readonly(fixture.mint, false),
        AccountMeta::new(source, false),
        AccountMeta::new(destination, false),
        AccountMeta::new_readonly(fixture.custody_authority, false),
        AccountMeta::new_readonly(Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID), false),
    ];
    assert_eq!(metas.len(), LIABILITY_BASIS_ACCOUNT_COUNT_V2);
    let mut data = action_bytes.to_vec();
    data.extend_from_slice(&request_bytes);
    let direct = Instruction {
        program_id: CLAIMS_PROGRAM_ID,
        accounts: metas.clone(),
        data: data.clone(),
    };
    let mut wrapper_metas = vec![AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false)];
    wrapper_metas.extend(metas);
    let mut wrapper_data = Vec::with_capacity(data.len() + 1);
    wrapper_data.push(1);
    wrapper_data.extend_from_slice(&data);
    let wrapper = Instruction {
        program_id: TEST_CALLER_PROGRAM_ID,
        accounts: wrapper_metas,
        data: wrapper_data,
    };
    BuiltAction {
        direct,
        wrapper,
        request,
        after: StateModel {
            market_revision: state.market_revision + 1,
            position_revision: state.position_revision + 1,
            custody_revision: state.custody_revision + 1,
            supplies,
            balances,
            hoard: candidate.hoard_after(),
        },
    }
}

async fn observed(context: &mut ProgramTestContext, key: Pubkey) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account query")
        .expect("existing account")
}

async fn snapshot(context: &mut ProgramTestContext, fixture: &Fixture) -> Snapshot {
    Snapshot {
        market: observed(context, fixture.market).await,
        position: observed(context, fixture.position).await,
        replay: observed(context, fixture.replay).await,
        external: observed(context, fixture.external).await,
        hoard: observed(context, fixture.hoard).await,
    }
}

fn token_amount(account: &Account) -> u64 {
    TokenAccount::parse(&account.data)
        .expect("token account")
        .amount
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes
            .get(offset..offset + 8)
            .expect("u64 field")
            .try_into()
            .expect("u64 width"),
    )
}

const PACKET_DATA_BYTES: usize = 1_232;

/// The extent of a legacy or v0 message once signed, checked against Solana's
/// packet maximum. `solana-program-test` submits no packet and cannot enforce
/// this itself -- Found31 was ten bytes over and survived every fixture test --
/// so the campaign measures it directly.
fn wire_extent(signatures: usize, message: &[u8]) -> usize {
    let extent = 1 + signatures * 64 + message.len();
    assert!(
        extent <= PACKET_DATA_BYTES,
        "the transaction serialises to {extent} bytes, past Solana's {PACKET_DATA_BYTES}-byte packet maximum"
    );
    extent
}

async fn process_legacy(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    label: &str,
) -> u64 {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = solana_transaction::Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let signature = transaction
        .signatures
        .first()
        .expect("signed ALT transaction")
        .to_string();
    let wire_bytes = wire_extent(transaction.signatures.len(), &transaction.message_data());
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("ALT lifecycle processing");
    let accepted = processed.result.is_ok();
    let failure = processed.result.err().map(|error| format!("{error:?}"));
    let (logs, units) = processed
        .metadata
        .map(|metadata| (metadata.log_messages, metadata.compute_units_consumed))
        .unwrap_or_default();
    dclutch_program_test_evidence::record(&TransactionEvidence {
        label,
        signature: &signature,
        slot,
        error: failure.as_deref(),
        logs: &logs,
        compute_units_consumed: Some(units),
        wire_bytes: Some(wire_bytes),
    })
    .expect("campaign evidence must be writable when the gauntlet asked for it");
    assert!(accepted, "ALT lifecycle must commit");
    units
}

async fn create_live_lookup_table(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    label_prefix: &str,
) -> (Pubkey, Vec<Pubkey>) {
    let payer = context.payer.pubkey();
    let fixture_signer = fixture_signer(instructions);
    let mut addresses = Vec::new();
    for instruction in instructions {
        if instruction.program_id != payer && !addresses.contains(&instruction.program_id) {
            addresses.push(instruction.program_id);
        }
        for meta in &instruction.accounts {
            if meta.pubkey != payer
                && Some(meta.pubkey) != fixture_signer
                && !addresses.contains(&meta.pubkey)
            {
                addresses.push(meta.pubkey);
            }
        }
    }
    let clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("Clock sysvar");
    context
        .warp_to_slot(clock.slot + 1)
        .expect("recent ALT slot");
    let (create, table) = create_lookup_table(payer, payer, clock.slot);
    process_legacy(
        context,
        create,
        &format!("{label_prefix}: create lookup table"),
    )
    .await;
    for (index, chunk) in addresses.chunks(20).enumerate() {
        process_legacy(
            context,
            extend_lookup_table(table, payer, Some(payer), chunk.to_vec()),
            &format!("{label_prefix}: extend lookup table {index}"),
        )
        .await;
    }
    let clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("post-extension Clock");
    context
        .warp_to_slot(clock.slot + 1)
        .expect("activate lookup table");
    (table, addresses)
}

fn fixture_signer(instructions: &[Instruction]) -> Option<Pubkey> {
    instructions
        .iter()
        .flat_map(|instruction| &instruction.accounts)
        .find(|meta| meta.is_signer)
        .map(|meta| meta.pubkey)
}

async fn submit_v0(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    instruction: Instruction,
    table: Pubkey,
    addresses: &[Pubkey],
    label: &str,
) -> Result<(bool, Vec<String>), BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let message = VersionedMessage::V0(
        v0::Message::try_compile(
            &context.payer.pubkey(),
            &[instruction],
            &[AddressLookupTableAccount {
                key: table,
                addresses: addresses.to_vec(),
            }],
            blockhash,
        )
        .expect("v0 message"),
    );
    let transaction = VersionedTransaction::try_new(message, &[&context.payer, &fixture.owner])
        .expect("signed v0 transaction");
    let signature = transaction
        .signatures
        .first()
        .ok_or(BanksClientError::ClientError("unsigned transaction"))?
        .to_string();
    let wire_bytes = wire_extent(
        transaction.signatures.len(),
        &transaction.message.serialize(),
    );
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await?;
    let accepted = processed.result.is_ok();
    let failure = processed
        .result
        .clone()
        .err()
        .map(|error| format!("{error:?}"));
    let (logs, units) = processed
        .metadata
        .map(|metadata| (metadata.log_messages, metadata.compute_units_consumed))
        .unwrap_or_default();
    dclutch_program_test_evidence::record(&TransactionEvidence {
        label,
        signature: &signature,
        slot,
        error: failure.as_deref(),
        logs: &logs,
        compute_units_consumed: Some(units),
        wire_bytes: Some(wire_bytes),
    })
    .expect("campaign evidence must be writable when the gauntlet asked for it");
    Ok((accepted, logs))
}

async fn assert_model(context: &mut ProgramTestContext, fixture: &Fixture, state: StateModel) {
    let actual = snapshot(context, fixture).await;
    assert_eq!(u64_at(&actual.market.data, 16), state.market_revision);
    assert_eq!(u64_at(&actual.position.data, 16), state.position_revision);
    for (index, expected) in state.supplies.into_iter().enumerate() {
        assert_eq!(u64_at(&actual.market.data, 256 + index * 8), expected);
    }
    for (index, expected) in state.balances.into_iter().enumerate() {
        assert_eq!(u64_at(&actual.position.data, 128 + index * 8), expected);
    }
    assert_eq!(
        CustodyReplayV1::decode(&actual.replay.data)
            .expect("Custody replay")
            .next_revision,
        state.custody_revision
    );
    assert_eq!(token_amount(&actual.hoard), state.hoard);
    assert_eq!(token_amount(&actual.external), MINT_SUPPLY - state.hoard);
}

/// The custom program error the runtime reported last, which is the one the
/// whole transaction failed with.
///
/// Read exactly the way the census reads it
/// (`tools/gauntlet/census/src/ledger.rs`, `reported_custom_code`), so a case
/// asserting a code here and a binding asserting one there cannot disagree
/// about which of several nested refusals counted.
fn reported_custom_code(logs: &[String]) -> Option<u32> {
    const MARKER: &str = "custom program error: 0x";
    logs.iter().rev().find_map(|line| {
        let index = line.find(MARKER)?;
        let digits: String = line
            .get(index + MARKER.len()..)?
            .chars()
            .take_while(char::is_ascii_hexdigit)
            .collect();
        u32::from_str_radix(&digits, 16).ok()
    })
}

/// The single encoded byte `LiabilityBasisActionV2` uses for the action kind.
///
/// Derived rather than restated: two actions differing only in `kind` are run
/// through the production encoder and the sole differing index is taken. The
/// offset itself is private to `liability_basis_v2.rs`, and a copy of it here
/// would be a mirror that survives the layout moving.
fn action_kind_offset() -> usize {
    let template = |kind| LiabilityBasisActionInputV2 {
        kind,
        custody_present: true,
        expected_market_revision: 0,
        expected_position_revision: 0,
        quantity: 1,
        claim_index: 0,
        expected_custody_revision: 0,
        request_nonce: 0,
    };
    let split = LiabilityBasisActionV2::new(template(LiabilityBasisActionKindV2::Split))
        .expect("canonical split action")
        .to_bytes();
    let merge = LiabilityBasisActionV2::new(template(LiabilityBasisActionKindV2::Merge))
        .expect("canonical merge action")
        .to_bytes();
    let differing: Vec<usize> = split
        .iter()
        .zip(merge.iter())
        .enumerate()
        .filter_map(|(index, (left, right))| (left != right).then_some(index))
        .collect();
    assert_eq!(
        differing.len(),
        1,
        "two actions differing only in kind must differ in exactly one encoded byte"
    );
    differing.first().copied().expect("the action kind byte")
}

/// Instruction data carrying the retired `TerminalRedeem` tag.
///
/// The tag cannot be built through `LiabilityBasisActionV2::new` at all, which
/// is precisely the fact under test, so the canonical encoding of an accepted
/// action is taken from production and its kind byte -- and only its kind byte
/// -- is set to the retired discriminant. Everything else on the wire stays
/// exactly what the program accepts, so the refusal is attributable to the tag.
fn retired_terminal_redeem_data(accepted: &Instruction) -> Vec<u8> {
    let offset = action_kind_offset();
    let mut data = accepted.data.clone();
    let byte = data.get_mut(offset).expect("encoded action kind");
    assert_ne!(
        *byte,
        LiabilityBasisActionKindV2::TerminalRedeem as u8,
        "the accepted action must not already carry the retired tag"
    );
    *byte = LiabilityBasisActionKindV2::TerminalRedeem as u8;
    data
}

/// `CustodySbfError::Instruction`, the refusal real Custody raises for an
/// external-source debit arriving on the V1 `CustodyRequestV1` wire.
///
/// Not imported from `dclutch-custody-sbf`: that is a second program crate and
/// this campaign has no business taking a code dependency on one to read a
/// discriminant. It is not restated by hand either, which is how it came to
/// say `0` for a while after `6cbcb3b` moved every program onto its registered
/// band. It comes from the band allocation instead (decision 0007) -- a shared
/// authority rather than another program -- and `Instruction` is by convention
/// the first variant, so it IS the base.
///
/// Provenance: `programs/dclutch-custody-sbf/src/lib.rs`, raised at the head of
/// `execute_transfer` whenever `request.source_compartment ==
/// CompartmentV1::External`. The census checks the same code against the
/// enumerated Custody taxonomy rather than against this constant
/// (`tools/gauntlet/claims-liability-basis-v2/bindings.json`).
const CUSTODY_INSTRUCTION_REFUSAL: u32 = dclutch_refusal_registry::CUSTODY_REFUSAL_BASE;

#[tokio::test]
async fn real_sbf_liability_basis_merge_lifecycle_and_hostile_joins_are_atomic() {
    let (test, fixture, initial) = fixture(false);
    let mut context = test.start_with_context().await;

    let canonical = build_action(
        &fixture,
        initial,
        LiabilityBasisActionKindV2::Merge,
        1,
        fixture.linked_basis_raw,
        fixture.linked_basis_staging,
    );
    assert_eq!(
        canonical.request.amount, SCALE,
        "one complete set withdraws exactly SCALE collateral"
    );
    assert_eq!(
        canonical.request.source_compartment,
        CompartmentV1::HoardPrincipal,
        "a merge debits the Hoard, which is the compartment the V1 wire still carries"
    );
    assert_eq!(
        canonical.request.destination_compartment,
        CompartmentV1::External
    );
    let unwind = build_action(
        &fixture,
        canonical.after,
        LiabilityBasisActionKindV2::Merge,
        2,
        fixture.linked_basis_raw,
        fixture.linked_basis_staging,
    );
    assert_eq!(unwind.after.supplies, [0, 0]);
    assert_eq!(unwind.after.hoard, 0);

    let wrong_product = build_action(
        &fixture,
        initial,
        LiabilityBasisActionKindV2::Merge,
        1,
        fixture.other_product_basis_raw,
        fixture.other_product_basis_staging,
    );
    let changed_payoff = build_action(
        &fixture,
        initial,
        LiabilityBasisActionKindV2::Merge,
        1,
        fixture.changed_payoff_basis_raw,
        fixture.changed_payoff_basis_staging,
    );

    // The split this route composes is the one real Custody refuses. It is
    // built canonically -- Claims accepts the request bytes and signs the CPI
    // -- so what the case records is a Custody boundary refusal, not a Claims
    // one.
    let retired_split = build_action(
        &fixture,
        initial,
        LiabilityBasisActionKindV2::Split,
        3,
        fixture.linked_basis_raw,
        fixture.linked_basis_staging,
    );
    assert_eq!(
        retired_split.request.source_compartment,
        CompartmentV1::External,
        "a split still debits External on the V1 wire, which is the refused shape"
    );

    let instructions = [
        canonical.direct.clone(),
        canonical.wrapper.clone(),
        unwind.direct.clone(),
        wrong_product.direct.clone(),
        changed_payoff.direct.clone(),
        retired_split.direct.clone(),
    ];
    let (table, addresses) = create_live_lookup_table(
        &mut context,
        &instructions,
        "claims liability-basis lifecycle",
    )
    .await;
    let before = snapshot(&mut context, &fixture).await;

    for (hostile, label) in [
        (
            wrong_product.direct,
            "claims liability-basis lifecycle: merge against a substituted Product basis",
        ),
        (
            changed_payoff.direct,
            "claims liability-basis lifecycle: merge against a changed payoff basis",
        ),
    ] {
        let (accepted, logs) = submit_v0(&mut context, &fixture, hostile, table, &addresses, label)
            .await
            .expect("hostile transaction");
        assert!(
            !accepted,
            "hostile finalized basis substitution must refuse"
        );
        assert_eq!(
            reported_custom_code(&logs),
            Some(LiabilityBasisSbfErrorV2::ProductLink as u32),
            "a substituted finalized basis is a Product-link refusal: {logs:#?}"
        );
        assert_eq!(snapshot(&mut context, &fixture).await, before);
    }

    let (accepted, logs) = submit_v0(
        &mut context,
        &fixture,
        retired_split.direct,
        table,
        &addresses,
        "claims liability-basis lifecycle: the composed external-debit split refuses at Custody",
    )
    .await
    .expect("retired split transaction");
    assert!(
        !accepted,
        "an external-source debit on the V1 Custody wire must refuse"
    );
    assert!(
        logs.iter().any(|log| log
            == &format!(
                "Program {CUSTODY_PROGRAM_ID} failed: custom program error: {CUSTODY_INSTRUCTION_REFUSAL:#x}"
            )),
        "real Custody must be the program that refuses the split: {logs:#?}"
    );
    assert_eq!(
        reported_custom_code(&logs),
        Some(CUSTODY_INSTRUCTION_REFUSAL),
        "the refusal Claims propagates is Custody's own"
    );
    assert_eq!(snapshot(&mut context, &fixture).await, before);

    let (accepted, logs) = submit_v0(
        &mut context,
        &fixture,
        canonical.wrapper,
        table,
        &addresses,
        "claims liability-basis lifecycle: caller refuses after a complete merge",
    )
    .await
    .expect("late rollback transaction");
    assert!(!accepted, "test wrapper must deliberately refuse late");
    assert!(
        logs.iter()
            .any(|log| log == &format!("Program {CUSTODY_PROGRAM_ID} success")),
        "real Custody must return before the late refusal: {logs:#?}"
    );
    assert!(
        logs.iter()
            .any(|log| log == &format!("Program {CLAIMS_PROGRAM_ID} success")),
        "real Claims must return before the late refusal"
    );
    assert_eq!(snapshot(&mut context, &fixture).await, before);

    let (accepted, logs) = submit_v0(
        &mut context,
        &fixture,
        canonical.direct,
        table,
        &addresses,
        "claims liability-basis lifecycle: canonical merge commits",
    )
    .await
    .expect("merge transaction");
    assert!(accepted, "real merge composition must commit: {logs:#?}");
    assert_model(&mut context, &fixture, canonical.after).await;

    let (accepted, logs) = submit_v0(
        &mut context,
        &fixture,
        unwind.direct,
        table,
        &addresses,
        "claims liability-basis lifecycle: a second merge unwinds the aggregate to zero",
    )
    .await
    .expect("unwind transaction");
    assert!(accepted, "the unwinding merge must commit: {logs:#?}");
    assert_model(&mut context, &fixture, unwind.after).await;
}

/// The retired terminal tag, and the fact that nothing replaces it here.
///
/// Both cases run against a TERMINAL Core Market -- the exact world the
/// retired action was designed for -- so neither refusal can be dismissed as a
/// wrong-phase accident of the fixture. Product V3 terminal settlement now
/// belongs entirely to `rational_terminal_v3`, whose real-ELF coverage lives in
/// `rational_representation_v2_program_test.rs`; this case exists to record
/// that `DCLLBX02` has no terminal action at all, not to re-test redemption.
#[tokio::test]
async fn real_sbf_retired_terminal_redeem_and_terminal_merge_both_refuse() {
    let (test, fixture, initial) = fixture(true);
    let mut context = test.start_with_context().await;

    let merge = build_action(
        &fixture,
        initial,
        LiabilityBasisActionKindV2::Merge,
        1,
        fixture.linked_basis_raw,
        fixture.linked_basis_staging,
    );
    let mut retired = merge.direct.clone();
    retired.data = retired_terminal_redeem_data(&merge.direct);
    assert_eq!(
        retired.data.len(),
        merge.direct.data.len(),
        "the retired tag differs from an accepted action in exactly one byte"
    );

    let instructions = [merge.direct.clone(), retired.clone()];
    let (table, addresses) = create_live_lookup_table(
        &mut context,
        &instructions,
        "claims liability-basis terminal",
    )
    .await;
    let before = snapshot(&mut context, &fixture).await;

    let (accepted, logs) = submit_v0(
        &mut context,
        &fixture,
        retired,
        table,
        &addresses,
        "claims liability-basis terminal: the retired TerminalRedeem tag refuses",
    )
    .await
    .expect("retired terminal transaction");
    assert!(!accepted, "the retired terminal tag must refuse");
    assert_eq!(
        reported_custom_code(&logs),
        Some(LiabilityBasisSbfErrorV2::Instruction as u32),
        "a retired action tag is refused as an instruction, at decode: {logs:#?}"
    );
    assert_eq!(snapshot(&mut context, &fixture).await, before);

    let (accepted, logs) = submit_v0(
        &mut context,
        &fixture,
        merge.direct,
        table,
        &addresses,
        "claims liability-basis terminal: a merge against a terminal Market refuses",
    )
    .await
    .expect("terminal merge transaction");
    assert!(
        !accepted,
        "Split and Merge both require an OPEN Core Market"
    );
    assert_eq!(
        reported_custom_code(&logs),
        Some(LiabilityBasisSbfErrorV2::ProductLink as u32),
        "a non-Open phase is a Product-link refusal: {logs:#?}"
    );
    assert_eq!(snapshot(&mut context, &fixture).await, before);
}

struct ProtocolFixture {
    release_set: [u8; 32],
    product_id: [u8; 32],
    semantic_basis_id: [u8; 32],
    core_market: Pubkey,
    market: Pubkey,
    position: Pubkey,
    admission: Pubkey,
    child_root: Pubkey,
    substituted_child_root: Pubkey,
    substituted_position: Pubkey,
    substituted_admission: Pubkey,
    activation_cache: Pubkey,
    basis_raw: Pubkey,
    basis_staging: Pubkey,
    product_raw: Pubkey,
    product_staging: Pubkey,
    claims_programdata: Pubkey,
    trading_programdata: Pubkey,
    core_programdata: Pubkey,
    position_rent: u64,
    admission_rent: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProtocolSnapshot {
    market: Account,
    position: Account,
    admission: Account,
}

fn add_prepaid_vacant(test: &mut ProgramTest, key: Pubkey, lamports: u64) {
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

fn protocol_fixture() -> (ProgramTest, ProtocolFixture) {
    let artifacts = artifacts();
    let basis = basis_artifacts();
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    for (name, id, elf) in [
        (
            "dclutch_claims_sbf",
            CLAIMS_PROGRAM_ID,
            artifacts.claims.as_slice(),
        ),
        (
            "dclutch_custody_sbf",
            CUSTODY_PROGRAM_ID,
            artifacts.custody.as_slice(),
        ),
        (
            "dclutch_registry_sbf",
            REGISTRY_PROGRAM_ID,
            artifacts.registry.as_slice(),
        ),
        (
            "dclutch_core_sbf",
            CORE_PROGRAM_ID,
            artifacts.core.as_slice(),
        ),
        (
            "dclutch_claims_liability_basis_test_caller_sbf",
            TEST_CALLER_PROGRAM_ID,
            artifacts.caller.as_slice(),
        ),
    ] {
        add_upgradeable_program(&mut test, name, id, elf);
    }
    let (release_set, cache_data) = activation_cache(&artifacts);
    let activation_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    add_account(&mut test, activation_cache, REGISTRY_PROGRAM_ID, cache_data);

    let child_root = Pubkey::new_from_array([0xd1; 32]);
    add_account(&mut test, child_root, TEST_CALLER_PROGRAM_ID, vec![1]);
    let (product_raw, product_staging, product_digest) = add_finalized_record(
        &mut test,
        PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1,
        &basis.product_bytes,
    );
    assert_eq!(product_digest, basis.product_id);
    let (basis_raw, basis_staging, _) = add_finalized_record(
        &mut test,
        LIABILITY_BASIS_SCHEMA_RELEASE_ID_V2,
        &basis.linked,
    );

    let mut core_identity = MarketIdentity {
        market_id: semantic_identity([1; 32]),
        realm_id: semantic_identity([0xd2; 32]),
        product_record: semantic_identity(product_digest),
        product_id: semantic_identity(basis.product_id),
        resolution_policy: semantic_identity([0x71; 32]),
        capability_manifest: semantic_identity([0x72; 32]),
        selected_release_set: semantic_identity(release_set),
        registry_program: semantic_identity(REGISTRY_PROGRAM_ID.to_bytes()),
        generation: GENERATION,
    };
    let core_market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(core_identity).as_slices(),
        &CORE_PROGRAM_ID,
    )
    .0;
    core_identity.market_id = semantic_identity(core_market.to_bytes());
    let core = CoreState {
        phase: CorePhase::Open,
        readiness: Readiness::Consumed,
        terminal_winner: 0,
        identity: core_identity,
        outstanding_capabilities: 1,
        rent_beneficiary: semantic_identity([0xd3; 32]),
        terminal_receipt: None,
    };
    add_account(
        &mut test,
        core_market,
        CORE_PROGRAM_ID,
        core.encode().expect("Core state").to_vec(),
    );

    let market = Pubkey::find_program_address(
        &ClaimsAggregateSeedsV1::new(core_market.to_bytes())
            .expect("Claims aggregate seeds")
            .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let mut market_data =
        vec![
            0_u8;
            MARKET_HEADER_BYTES
                + usize::try_from(CLAIM_COUNT).expect("small width") * 3 * SCALAR_BYTES
        ];
    initialize_market(
        &mut market_data,
        core_market.to_bytes(),
        release_set,
        REGISTRY_PROGRAM_ID.to_bytes(),
        CLAIM_COUNT,
        EconomicPhase::Open,
        0,
    )
    .expect("Claims aggregate");
    add_account(&mut test, market, CLAIMS_PROGRAM_ID, market_data);

    let position = Pubkey::find_program_address(
        &ClaimsPositionSeedsV1::new(core_market.to_bytes(), child_root.to_bytes())
            .expect("Claims Position seeds")
            .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let admission = Pubkey::find_program_address(
        &[
            PROTOCOL_POSITION_ADMISSION_SEED_V2,
            core_market.as_ref(),
            child_root.as_ref(),
        ],
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let position_width = dclutch_economic_slice_kernel::POSITION_HEADER_BYTES
        + usize::try_from(CLAIM_COUNT).expect("small width") * 2 * SCALAR_BYTES;
    let position_rent = Rent::default().minimum_balance(position_width);
    let admission_rent = Rent::default().minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2);
    add_prepaid_vacant(&mut test, position, position_rent);
    add_prepaid_vacant(&mut test, admission, admission_rent);
    let substituted_child_root = Pubkey::new_from_array([0xd6; 32]);
    add_account(
        &mut test,
        substituted_child_root,
        system_program::ID,
        Vec::new(),
    );
    let substituted_position = Pubkey::find_program_address(
        &ClaimsPositionSeedsV1::new(core_market.to_bytes(), substituted_child_root.to_bytes())
            .expect("substituted Position seeds")
            .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let substituted_admission = Pubkey::find_program_address(
        &[
            PROTOCOL_POSITION_ADMISSION_SEED_V2,
            core_market.as_ref(),
            substituted_child_root.as_ref(),
        ],
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    add_prepaid_vacant(&mut test, substituted_position, position_rent);
    add_prepaid_vacant(&mut test, substituted_admission, admission_rent);

    (
        test,
        ProtocolFixture {
            release_set,
            product_id: basis.product_id,
            semantic_basis_id: basis.semantic_id,
            core_market,
            market,
            position,
            admission,
            child_root,
            substituted_child_root,
            substituted_position,
            substituted_admission,
            activation_cache,
            basis_raw,
            basis_staging,
            product_raw,
            product_staging,
            claims_programdata: programdata_address(CLAIMS_PROGRAM_ID),
            trading_programdata: programdata_address(TEST_CALLER_PROGRAM_ID),
            core_programdata: programdata_address(CORE_PROGRAM_ID),
            position_rent,
            admission_rent,
        },
    )
}

fn protocol_request(fixture: &ProtocolFixture) -> ProtocolPositionRequestV2 {
    ProtocolPositionRequestV2 {
        action: ProtocolPositionActionV2::Admit,
        owner_kind: ProtocolPositionOwnerKindV2::TradingRecord,
        presence: ProtocolPositionPresenceV2::Vacant,
        release_set: fixture.release_set,
        market: fixture.core_market.to_bytes(),
        position_owner: fixture.child_root.to_bytes(),
        parent_request_digest: [0xd4; 32],
        rent_credit: [0xd5; 32],
        rent_program: [0xd7; 32],
        generation: GENERATION,
        expected_market_revision: 0,
        expected_position_revision: 0,
        observed_position_lamports: fixture.position_rent,
        observed_admission_lamports: fixture.admission_rent,
        position_rent_principal: fixture.position_rent,
        admission_rent_principal: fixture.admission_rent,
        capability_descriptor: [0; 32],
        capability_outcome: 0,
    }
}

fn protocol_wrapper_instruction(
    fixture: &ProtocolFixture,
    request: ProtocolPositionRequestV2,
    fail_after: bool,
) -> Instruction {
    protocol_wrapper_instruction_for(
        fixture,
        request,
        fail_after,
        fixture.position,
        fixture.admission,
        fixture.child_root,
    )
}

fn protocol_wrapper_instruction_for(
    fixture: &ProtocolFixture,
    request: ProtocolPositionRequestV2,
    fail_after: bool,
    position: Pubkey,
    admission: Pubkey,
    child_root: Pubkey,
) -> Instruction {
    let request_bytes = request.to_bytes().expect("protocol Position request");
    let authority = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::new(
            ContentId::new(request.release_set).expect("release set"),
            request.market,
            ExecutionRoleV1::Trading,
            request.position_owner,
            hash(&request_bytes).to_bytes(),
        )
        .expect("Trading authority seeds")
        .as_slices(),
        &TEST_CALLER_PROGRAM_ID,
    )
    .0;
    let forwarded = vec![
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new_readonly(fixture.market, false),
        AccountMeta::new(position, false),
        AccountMeta::new(admission, false),
        AccountMeta::new_readonly(fixture.basis_raw, false),
        AccountMeta::new_readonly(fixture.basis_staging, false),
        AccountMeta::new_readonly(fixture.product_raw, false),
        AccountMeta::new_readonly(fixture.product_staging, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(fixture.core_market, false),
        AccountMeta::new_readonly(fixture.activation_cache, false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        AccountMeta::new_readonly(TEST_CALLER_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.trading_programdata, false),
        AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.claims_programdata, false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.core_programdata, false),
        AccountMeta::new_readonly(child_root, false),
    ];
    assert_eq!(
        forwarded.len(),
        PROTOCOL_POSITION_ADMIT_ACCOUNT_COUNT_V2 - 6
    );
    let mut accounts = vec![AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false)];
    accounts.extend(forwarded);
    let mut data = Vec::with_capacity(request_bytes.len() + 1);
    data.push(u8::from(fail_after));
    data.extend_from_slice(&request_bytes);
    Instruction {
        program_id: TEST_CALLER_PROGRAM_ID,
        accounts,
        data,
    }
}

async fn protocol_snapshot(
    context: &mut ProgramTestContext,
    fixture: &ProtocolFixture,
) -> ProtocolSnapshot {
    ProtocolSnapshot {
        market: observed(context, fixture.market).await,
        position: observed(context, fixture.position).await,
        admission: observed(context, fixture.admission).await,
    }
}

async fn submit_payer_v0(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    table: Pubkey,
    addresses: &[Pubkey],
) -> Result<(bool, Vec<String>, Option<(Pubkey, Vec<u8>)>), BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let message = VersionedMessage::V0(
        v0::Message::try_compile(
            &context.payer.pubkey(),
            &[instruction],
            &[AddressLookupTableAccount {
                key: table,
                addresses: addresses.to_vec(),
            }],
            blockhash,
        )
        .expect("v0 message"),
    );
    let transaction = VersionedTransaction::try_new(message, &[&context.payer])
        .expect("payer-signed v0 transaction");
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await?;
    let (logs, returned) = processed
        .metadata
        .map(|metadata| {
            (
                metadata.log_messages,
                metadata
                    .return_data
                    .map(|value| (value.program_id, value.data)),
            )
        })
        .unwrap_or_default();
    Ok((processed.result.is_ok(), logs, returned))
}

#[tokio::test]
#[ignore = "superseded by the Product Runtime V2 protocol Position campaign"]
async fn real_sbf_protocol_position_admission_is_zero_atomic_and_release_bound() {
    let (test, fixture) = protocol_fixture();
    let mut context = test.start_with_context().await;
    let request = protocol_request(&fixture);
    let positive = protocol_wrapper_instruction(&fixture, request, false);
    let late = protocol_wrapper_instruction(&fixture, request, true);
    let mut wrong_release = request;
    wrong_release.release_set = [0xe1; 32];
    let wrong_release = protocol_wrapper_instruction(&fixture, wrong_release, false);
    let mut wrong_market = request;
    wrong_market.market = [0xe2; 32];
    let wrong_market = protocol_wrapper_instruction(&fixture, wrong_market, false);
    let mut wrong_owner_request = request;
    wrong_owner_request.position_owner = fixture.substituted_child_root.to_bytes();
    let wrong_owner = protocol_wrapper_instruction_for(
        &fixture,
        wrong_owner_request,
        false,
        fixture.substituted_position,
        fixture.substituted_admission,
        fixture.substituted_child_root,
    );
    let instructions = [
        positive.clone(),
        late.clone(),
        wrong_release.clone(),
        wrong_market.clone(),
        wrong_owner.clone(),
    ];
    let (table, addresses) = create_live_lookup_table(
        &mut context,
        &instructions,
        "claims protocol-position admission",
    )
    .await;
    let before = protocol_snapshot(&mut context, &fixture).await;

    for hostile in [wrong_release, wrong_market, wrong_owner] {
        let (accepted, _, _) = submit_payer_v0(&mut context, hostile, table, &addresses)
            .await
            .expect("hostile protocol Position transaction");
        assert!(!accepted, "substituted owner/Market/release must refuse");
        assert_eq!(protocol_snapshot(&mut context, &fixture).await, before);
    }
    let (accepted, logs, _) = submit_payer_v0(&mut context, late, table, &addresses)
        .await
        .expect("late protocol Position rollback");
    assert!(
        !accepted,
        "test Trading caller must deliberately refuse late"
    );
    assert!(
        logs.iter()
            .any(|log| log == &format!("Program {CLAIMS_PROGRAM_ID} success")),
        "Claims must finish allocation and return before the late refusal"
    );
    assert_eq!(protocol_snapshot(&mut context, &fixture).await, before);

    let (accepted, _, returned) = submit_payer_v0(&mut context, positive, table, &addresses)
        .await
        .expect("positive protocol Position transaction");
    assert!(accepted, "protocol Position admission must commit");
    let (producer, receipt_bytes) = returned.expect("immediate Claims receipt");
    assert_eq!(producer, CLAIMS_PROGRAM_ID);
    assert_eq!(receipt_bytes.len(), PROTOCOL_POSITION_ADMISSION_BYTES_V2);
    let receipt = ProtocolPositionAdmissionV2::decode_receipt(&receipt_bytes)
        .expect("protocol Position receipt");
    assert_eq!(receipt.market(), fixture.core_market.to_bytes());
    assert_eq!(receipt.position_owner(), fixture.child_root.to_bytes());
    assert_eq!(receipt.product_record_digest(), fixture.product_id);
    assert_eq!(receipt.semantic_basis_id(), fixture.semantic_basis_id);
    assert_eq!(receipt.market_revision(), 0);
    assert_eq!(receipt.outcome_count(), CLAIM_COUNT);
    let after = protocol_snapshot(&mut context, &fixture).await;
    assert_eq!(
        after.market, before.market,
        "aggregate must remain byte-exact"
    );
    assert_eq!(after.position.owner, CLAIMS_PROGRAM_ID);
    assert_eq!(after.admission.owner, CLAIMS_PROGRAM_ID);
    assert_eq!(
        after.position.data.len(),
        dclutch_economic_slice_kernel::POSITION_HEADER_BYTES
            + usize::try_from(CLAIM_COUNT).expect("small width") * 2 * SCALAR_BYTES
    );
    assert!(
        after
            .position
            .data
            .get(96..)
            .expect("Position vectors")
            .iter()
            .all(|byte| *byte == 0),
        "native and materialized inventory must initialize to exact zero"
    );

    let committed = protocol_snapshot(&mut context, &fixture).await;
    let replay = protocol_wrapper_instruction(&fixture, request, false);
    let (accepted, _, _) = submit_payer_v0(&mut context, replay, table, &addresses)
        .await
        .expect("preexisting Position transaction");
    assert!(!accepted, "preexisting Claims Position must refuse");
    assert_eq!(protocol_snapshot(&mut context, &fixture).await, committed);
}
