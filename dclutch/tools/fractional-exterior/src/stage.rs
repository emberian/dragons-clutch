//! Stage the exact Fractional fixture as preloadable validator accounts.
//!
//! The ProgramTest campaign plants its fixture through `ProgramTest::add_account`.
//! A real validator has no such door, so the same accounts are emitted here as
//! genesis account files and the programs are deployed as upgradeable programs.
//! Nothing about the accounts themselves differs -- that is the point. The
//! exterior is only evidence if it stages the geometry the ProgramTest campaign
//! proved, so the staged bytes are digested and the digest is journalled.

use solana_program::{hash::hash, pubkey::Pubkey};

use dclutch_capability_program_contract::{CapabilityRootHeaderV1, SelectedRecordBumpsV1};
use dclutch_core_contract::ContentId;
use dclutch_fractional_claim_contract::{
    FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4, FractionalExposureActionV2,
    FractionalExposureRequestInputV2, FractionalExposureRequestV2, FractionalRootInputV1,
    FractionalRootV1,
};
use dclutch_fractional_claim_kernel::{
    FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FractionalExposureTermsInputV2,
    encode_fractional_exposure_terms_v2, fractional_exposure_terms_bytes_v2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, CallerAuthoritySeedsV1, CapabilityExecutionSelectionV1,
    ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1, ProgramIdentityV1,
};
use dclutch_token_svm::{
    TOKEN_2022_PROGRAM_ID, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2, TokenBehaviorSelectionV2,
};

use crate::narrow_fixture::{
    NarrowFixtureInputV2, NarrowFixtureV2, NarrowRecordV2, compile_narrow_fixture_v2,
};

/// Program identities, matched to the ProgramTest campaign so a refusal seen in
/// one is greppable in the other.
pub const CLAIMS: Pubkey = Pubkey::new_from_array([0xa1; 32]);
/// Registry program.
pub const REGISTRY: Pubkey = Pubkey::new_from_array([0xa2; 32]);
/// Core program.
pub const CORE: Pubkey = Pubkey::new_from_array([0xa3; 32]);
/// Test caller standing in for Trading.
pub const CALLER: Pubkey = Pubkey::new_from_array([0xa8; 32]);

const REALM_ID: [u8; 32] = [0x61; 32];
const CUSTODY_CONTEXT: [u8; 32] = [0x62; 32];
const GENERATION: u64 = 37;
const ROOT_REVISION: u64 = 1;
const DENOMINATOR: u64 = 10;
const WRAP_NATIVE_CLAIMS: u64 = 7;
const OUTCOME: u32 = 0;
const ACTOR_FUNDED_BALANCE: u64 = 1_000;
const RENT_CREDIT: Pubkey = Pubkey::new_from_array([0x65; 32]);
const GRAPH_ID: [u8; 32] = [0x7c; 32];
const EXPOSURE_ID: [u8; 32] = [0x7a; 32];
const HOLDER_TOKEN: Pubkey = Pubkey::new_from_array([0x78; 32]);
/// Representation width the exterior runs at, inside the settleable bound.
pub const WIDTH: usize = 8;

/// One account to preload at genesis.
#[derive(Clone, Debug)]
pub struct StagedAccount {
    /// Account address.
    pub key: Pubkey,
    /// Owning program.
    pub owner: Pubkey,
    /// Exact account bytes.
    pub data: Vec<u8>,
}

/// One account reference inside a submitted instruction.
#[derive(Clone, Copy, Debug)]
pub struct Meta {
    /// Account address.
    pub key: Pubkey,
    /// Must sign.
    pub signer: bool,
    /// May be written.
    pub writable: bool,
}

/// One executable Fractional action.
#[derive(Clone, Debug)]
pub struct StagedAction {
    /// Stable journal label.
    pub name: &'static str,
    /// Exact caller wrapper bytes.
    pub data: Vec<u8>,
    /// Exact ordered account frame, caller program first.
    pub metas: Vec<Meta>,
}

/// The complete staged exterior.
#[derive(Clone, Debug)]
pub struct Staged {
    /// Accounts to preload at genesis.
    pub accounts: Vec<StagedAccount>,
    /// Ordered executable actions.
    pub actions: Vec<StagedAction>,
    /// Shard Mint, observed for supply.
    pub shard_mint: Pubkey,
    /// Holder token account, observed for amount.
    pub holder_token: Pubkey,
    /// Actor Position, observed for native Claims.
    pub actor_position: Pubkey,
    /// Reserve Position, observed for locked Claims.
    pub reserve_position: Pubkey,
}

fn finalized(owner: Pubkey, schema: [u8; 32], bytes: Vec<u8>) -> NarrowRecordV2 {
    let digest = hash(&bytes).to_bytes();
    NarrowRecordV2 {
        owner,
        schema,
        digest,
        raw: Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &owner).0,
        staging: Pubkey::find_program_address(
            &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
            &owner,
        )
        .0,
        bytes,
    }
}

fn identity(key: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(key.to_bytes()).expect("nonzero program identity")
}

fn programdata(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &loader()).0
}

/// BPF upgradeable loader.
pub fn loader() -> Pubkey {
    Pubkey::new_from_array([
        2, 168, 246, 145, 78, 136, 161, 176, 226, 16, 21, 62, 247, 99, 174, 43, 0, 194, 185, 61,
        22, 193, 36, 210, 192, 83, 122, 16, 4, 128, 0, 0,
    ])
}

fn release(program: Pubkey, seed: u8, elf: &[u8]) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        identity(program),
        identity(loader()),
        programdata(program).to_bytes(),
        ContentId::new([seed; 32]).expect("semantic release"),
        hash(elf).to_bytes(),
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("artifact release")
}

fn artifact_id(value: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&value.to_bytes()).to_bytes()).expect("artifact ID")
}

fn binding(value: ArtifactReleaseV1) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(value.program(), artifact_id(value))
}

fn activation_input(value: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
    let observation = DeploymentObservationV1::new(
        value.program().to_bytes(),
        loader().to_bytes(),
        true,
        value.programdata(),
        loader().to_bytes(),
        false,
        value.programdata(),
        loader().to_bytes(),
        value.deployment_slot(),
        value.elf_digest(),
        value.upgrade_authority(),
    )
    .expect("deployment observation");
    ArtifactActivationInputV1::new(artifact_id(value), value, observation)
}

/// Elf bytes for each deployed program, by role.
pub struct Elves<'a> {
    /// Claims program.
    pub claims: &'a [u8],
    /// Registry program.
    pub registry: &'a [u8],
    /// Core program.
    pub core: &'a [u8],
    /// Test caller standing in for Trading.
    pub caller: &'a [u8],
}

fn activation_cache(elves: &Elves<'_>) -> ([u8; 32], Vec<u8>) {
    let core = release(CORE, 0x31, elves.core);
    let claims = release(CLAIMS, 0x32, elves.claims);
    let trading = release(CALLER, 0x33, elves.caller);
    let set = ExecutionReleaseSetV1::new(
        binding(core),
        binding(claims),
        binding(trading),
        binding(claims),
        binding(claims),
    )
    .expect("release set");
    let id = hash(&set.to_bytes()).to_bytes();
    let content = ContentId::new(id).expect("release-set ID");
    let mut bytes = vec![0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut bytes, content).expect("initialize cache");
    for (role, artifact) in [
        (ExecutionRoleV1::Core, core),
        (ExecutionRoleV1::Claims, claims),
        (ExecutionRoleV1::Trading, trading),
        (ExecutionRoleV1::Resolution, claims),
        (ExecutionRoleV1::Custody, claims),
    ] {
        activate_execution_role_into_v1(
            &mut bytes,
            content,
            &set,
            role,
            &activation_input(artifact),
        )
        .expect("activate role");
    }
    ActivatedExecutionReleaseSetV1::decode(&bytes).expect("complete cache");
    (id, bytes)
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    output[offset..offset + input.len()].copy_from_slice(input);
}

fn mint_bytes(controller: Pubkey, supply: u64) -> Vec<u8> {
    const TLV_START: usize = 166;
    let mut bytes = vec![0_u8; TLV_START];
    put(&mut bytes, 0, &1_u32.to_le_bytes());
    put(&mut bytes, 4, controller.as_ref());
    put(&mut bytes, 36, &supply.to_le_bytes());
    bytes[45] = 1;
    bytes[165] = 1;
    for extension in [3_u16, 28_u16] {
        bytes.extend_from_slice(&extension.to_le_bytes());
        bytes.extend_from_slice(&32_u16.to_le_bytes());
        bytes.extend_from_slice(controller.as_ref());
    }
    bytes
}

fn token_account_bytes(mint: Pubkey, owner: Pubkey, amount: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; 165];
    put(&mut bytes, 0, mint.as_ref());
    put(&mut bytes, 32, owner.as_ref());
    put(&mut bytes, 64, &amount.to_le_bytes());
    bytes[108] = 1;
    bytes
}

/// Stage the open-market Fractional exterior: Wrap then WholeUnwrap.
pub fn stage(elves: &Elves<'_>, actor: Pubkey) -> Staged {
    let (release_set, cache) = activation_cache(elves);
    let cache_key =
        Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &release_set], &REGISTRY).0;

    let probe = compile(release_set, actor, Pubkey::new_from_array([0xef; 32]));
    let core_market = probe.core_market;

    let shard_mints: Vec<[u8; 32]> = (0..WIDTH)
        .map(|index| {
            let mut bytes = [0x77_u8; 32];
            let index = u32::try_from(index).expect("coordinate");
            bytes[0..4].copy_from_slice(&index.to_le_bytes());
            bytes
        })
        .collect();
    let shard_mint = Pubkey::new_from_array(shard_mints[OUTCOME as usize]);
    let behavior = finalized(
        REGISTRY,
        TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
        TokenBehaviorSelectionV2::new(REALM_ID, release_set)
            .expect("behavior")
            .to_bytes()
            .to_vec(),
    );
    let width = fractional_exposure_terms_bytes_v2(shard_mints.len()).expect("terms width");
    let mut scratch = vec![0_u8; width];
    let mut terms_bytes = vec![0_u8; width];
    encode_fractional_exposure_terms_v2(
        FractionalExposureTermsInputV2 {
            market: core_market.to_bytes(),
            product_record: probe.product.digest,
            result_domain: probe.result_domain.digest,
            release_set,
            token_program: TOKEN_2022_PROGRAM_ID,
            token_behavior: behavior.digest,
            exposure_id: EXPOSURE_ID,
            product_basis: probe.linked_basis.digest,
            representation_basis: probe.semantic_basis_id,
            graph_id: GRAPH_ID,
            product_width: probe.outcome_count,
            denominator: DENOMINATOR,
            shard_mints: &shard_mints,
        },
        &mut scratch,
        &mut terms_bytes,
    )
    .expect("terms");
    let terms = finalized(
        REGISTRY,
        FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
        terms_bytes,
    );

    let selection = CapabilityExecutionSelectionV1::new(
        0,
        ContentId::new([0x81; 32]).expect("manifest"),
        ContentId::new(dclutch_fractional_claim_contract::FRACTIONAL_CAPABILITY_KIND_ID_V1)
            .expect("kind"),
        ContentId::new([0x83; 32]).expect("capability release"),
        ContentId::new(terms.digest).expect("config"),
    )
    .expect("selection");
    let header = CapabilityRootHeaderV1::new(
        ContentId::new(release_set).expect("release set"),
        core_market.to_bytes(),
        GENERATION,
        selection,
        SelectedRecordBumpsV1::default(),
    )
    .expect("root header");
    let (root, bump) = Pubkey::find_program_address(&header.seeds().as_slices(), &CALLER);
    let shared = compile(release_set, actor, root);
    assert_eq!(shared.core_market, core_market, "Market must not move");

    let state = FractionalRootV1::new(FractionalRootInputV1 {
        bump,
        terms: terms.digest,
        market: core_market.to_bytes(),
        rent_beneficiary: actor.to_bytes(),
        revision: ROOT_REVISION,
        historical_rent_principal: 1,
    })
    .expect("root state");
    let mut root_bytes = vec![0_u8; FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4];
    root_bytes.copy_from_slice(&header.to_bytes());
    root_bytes.extend_from_slice(&state.to_bytes());

    let mut accounts = vec![
        StagedAccount {
            key: cache_key,
            owner: REGISTRY,
            data: cache,
        },
        StagedAccount {
            key: shared.core_market,
            owner: CORE,
            data: shared.core_state.clone(),
        },
        StagedAccount {
            key: shared.claims_market,
            owner: CLAIMS,
            data: shared.claims_market_bytes.clone(),
        },
        StagedAccount {
            key: root,
            owner: CALLER,
            data: root_bytes,
        },
        StagedAccount {
            key: shard_mint,
            owner: token_program(),
            data: mint_bytes(root, 0),
        },
        StagedAccount {
            key: HOLDER_TOKEN,
            owner: token_program(),
            data: token_account_bytes(shard_mint, actor, 0),
        },
        StagedAccount {
            key: RENT_CREDIT,
            owner: system(),
            data: Vec::new(),
        },
    ];
    for record in [
        &shared.product,
        &shared.result_domain,
        &shared.portfolio,
        &shared.linked_basis,
        &shared.exposure,
        &terms,
        &behavior,
    ] {
        accounts.push(StagedAccount {
            key: record.raw,
            owner: record.owner,
            data: record.bytes.clone(),
        });
        accounts.push(StagedAccount {
            key: record.staging,
            owner: system(),
            data: Vec::new(),
        });
    }
    for position in shared.ordered_positions() {
        accounts.push(StagedAccount {
            key: position.account,
            owner: CLAIMS,
            data: position.bytes.clone(),
        });
    }

    let mut actions = Vec::new();
    for (name, action) in [
        ("wrap", FractionalExposureActionV2::Wrap),
        ("whole-unwrap", FractionalExposureActionV2::WholeUnwrap),
    ] {
        let (source, destination, quantity) = match action {
            FractionalExposureActionV2::Wrap => {
                ([0; 32], HOLDER_TOKEN.to_bytes(), WRAP_NATIVE_CLAIMS)
            }
            _ => (
                HOLDER_TOKEN.to_bytes(),
                [0; 32],
                WRAP_NATIVE_CLAIMS * DENOMINATOR,
            ),
        };
        let request = FractionalExposureRequestV2::new(
            action,
            FractionalExposureRequestInputV2 {
                release_set,
                market: core_market.to_bytes(),
                product_record: shared.product.digest,
                result_domain: shared.result_domain.digest,
                terms: terms.digest,
                token_behavior: behavior.digest,
                exposure: EXPOSURE_ID,
                owner: actor.to_bytes(),
                source_token_account: source,
                destination_token_account: destination,
                terminal_digest: [0; 32],
                expected_revision: ROOT_REVISION,
                quantity,
                representation_coordinate: OUTCOME,
            },
        )
        .expect("request");
        let request_bytes = request.to_bytes().expect("request bytes");
        let authority = Pubkey::find_program_address(
            &CallerAuthoritySeedsV1::from_bytes(
                release_set,
                core_market.to_bytes(),
                ExecutionRoleV1::Trading,
                terms.digest,
                hash(&request_bytes).to_bytes(),
            )
            .expect("caller seeds")
            .as_slices(),
            &CALLER,
        )
        .0;
        accounts.push(StagedAccount {
            key: authority,
            owner: system(),
            data: Vec::new(),
        });
        let mut data = Vec::with_capacity(1 + request_bytes.len());
        data.push(0);
        data.extend_from_slice(&request_bytes);
        let [position_0, position_1] = shared.ordered_positions();
        let mut metas = vec![Meta {
            key: CLAIMS,
            signer: false,
            writable: false,
        }];
        for (key, signer, writable) in [
            (authority, false, false),
            (shared.claims_market, false, true),
            (shared.linked_basis.raw, false, false),
            (shared.linked_basis.staging, false, false),
            (shared.product.raw, false, false),
            (shared.product.staging, false, false),
            (shared.result_domain.raw, false, false),
            (shared.result_domain.staging, false, false),
            (shared.portfolio.raw, false, false),
            (shared.portfolio.staging, false, false),
            (rent_sysvar(), false, false),
            (shared.core_market, false, false),
            (cache_key, false, false),
            (REGISTRY, false, false),
            (CALLER, false, false),
            (programdata(CALLER), false, false),
            (CLAIMS, false, false),
            (programdata(CLAIMS), false, false),
            (CORE, false, false),
            (programdata(CORE), false, false),
            (position_0.account, false, true),
            (position_1.account, false, true),
            (terms.raw, false, false),
            (terms.staging, false, false),
            (behavior.raw, false, false),
            (behavior.staging, false, false),
            (root, false, true),
            (actor, true, false),
            (shard_mint, false, true),
            (HOLDER_TOKEN, false, true),
            (token_program(), false, false),
        ] {
            metas.push(Meta {
                key,
                signer,
                writable,
            });
        }
        actions.push(StagedAction { name, data, metas });
    }

    Staged {
        accounts,
        actions,
        shard_mint,
        holder_token: HOLDER_TOKEN,
        actor_position: shared.actor_position.account,
        reserve_position: shared.reserve_position.account,
    }
}

fn compile(release_set: [u8; 32], actor: Pubkey, reserve: Pubkey) -> NarrowFixtureV2 {
    compile_narrow_fixture_v2(NarrowFixtureInputV2 {
        outcome_count: WIDTH,
        registry_program: REGISTRY,
        core_program: CORE,
        claims_program: CLAIMS,
        release_set,
        realm_id: REALM_ID,
        custody_context: CUSTODY_CONTEXT,
        generation: GENERATION,
        actor_owner: actor,
        reserve_owner: reserve,
        funded_coordinate: OUTCOME as usize,
        funded_balance: ACTOR_FUNDED_BALANCE,
        reserve_balance: 0,
        position_revision: 0,
        terminal: None,
        rent_beneficiary: RENT_CREDIT,
        graph_id: GRAPH_ID,
        exposure_id: EXPOSURE_ID,
    })
    .expect("narrow fixture at the exterior width")
}

/// Token-2022 program.
pub fn token_program() -> Pubkey {
    Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID)
}

fn system() -> Pubkey {
    Pubkey::new_from_array([0; 32])
}

fn rent_sysvar() -> Pubkey {
    Pubkey::new_from_array([
        6, 167, 213, 23, 25, 44, 92, 81, 33, 140, 201, 76, 61, 74, 241, 127, 88, 218, 238, 8, 155,
        161, 253, 68, 227, 219, 217, 138, 0, 0, 0, 0,
    ])
}
