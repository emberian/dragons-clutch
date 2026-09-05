//! Stage the exact Fractional fixture as preloadable validator accounts.
//!
//! The ProgramTest campaign plants its fixture through `ProgramTest::add_account`.
//! A real validator has no such door, so the same accounts are emitted here as
//! genesis account files and the programs are deployed as upgradeable programs.
//! Nothing about the accounts themselves differs -- that is the point. The
//! exterior is only evidence if it stages the geometry the ProgramTest campaign
//! proved, so the staged bytes are digested and the digest is journalled.

use solana_program::{hash::hash, pubkey::Pubkey};

use dclutch_market::capability_program::{CapabilityRootHeaderV1, SelectedRecordBumpsV1};
use dclutch_core_contract::ContentId;
use dclutch_claims::fractional::{
    FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4, FractionalExposureActionV2,
    FractionalExposureRequestInputV2, FractionalExposureRequestV2, FractionalRootInputV1,
    FractionalRootV1,
};
use dclutch_claims::fractional_kernel::{
    FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FRACTIONAL_SELECTION_CONFIG_BYTES_V1,
    FractionalExposureTermsAdmissionV2, FractionalExposureTermsInputV2, FractionalExposureTermsV2,
    encode_fractional_exposure_terms_v2, encode_fractional_selection_config_v1,
    fractional_exposure_terms_bytes_v2, fractional_selection_config_from_terms_v1,
};
use dclutch_product::payoff::price_gate_v1::{
    PRICE_GATE_ATOM_COUNT_OFFSET_V1, PRICE_GATE_DEGREE_OFFSET_V1,
    PRICE_GATE_DENOMINATORS_OFFSET_V1, PRICE_GATE_MAGIC_OFFSET_V1, PRICE_GATE_MAGIC_V1,
    PRICE_GATE_MASS_OFFSET_V1, PRICE_GATE_NUMERATORS_OFFSET_V1, PRICE_GATE_PRICES_OFFSET_V1,
    PRICE_GATE_PROFILE_OFFSET_V1, PRICE_GATE_PROFILE_V1, PRICE_GATE_REQUEST_BYTES_V1,
    PRICE_GATE_SCALE_OFFSET_V1, PRICE_GATE_SCHEMA_VERSION_V1, PRICE_GATE_VERSION_OFFSET_V1,
    PRICE_GATE_WEIGHTS_OFFSET_V1, PRICE_GATE_WIDTH_OFFSET_V1,
};
use dclutch_market::realm::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_registry::release_set::{
    ArtifactReleaseIdV1, CallerAuthoritySeedsV1, CapabilityExecutionSelectionV1,
    ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1, ProgramIdentityV1,
};
use dclutch_custody::token_svm::{
    PRODUCTION_ADAPTER_RELEASES, TOKEN_2022_PROGRAM_ID, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
    TokenBehaviorSelectionV2,
};

use crate::narrow_fixture::{
    NarrowBasisInputV3, NarrowFixtureInputV2, NarrowFixtureV2, NarrowRecordV2,
    NarrowSplineBasisInputV3, compile_narrow_fixture_v3, finalized,
};

/// Program identities, matched to the ProgramTest campaign so a refusal seen in
/// one is greppable in the other.
pub const CLAIMS: Pubkey = Pubkey::new_from_array([0xa1; 32]);
/// Registry program.
pub const REGISTRY: Pubkey = Pubkey::new_from_array([0xa2; 32]);
/// Core program.
pub const CORE: Pubkey = Pubkey::new_from_array([0xa3; 32]);
/// Custody program.
pub const CUSTODY: Pubkey = Pubkey::new_from_array([0xa4; 32]);
/// Test caller standing in for Trading.
pub const CALLER: Pubkey = Pubkey::new_from_array([0xa9; 32]);

/// Collateral mint pinned by the canonical Realm.
pub const COLLATERAL_MINT: Pubkey = Pubkey::new_from_array([0x74; 32]);
const CUSTODY_CONTEXT: [u8; 32] = [0x62; 32];
const GENERATION: u64 = 37;
const ROOT_REVISION: u64 = 1;
const DENOMINATOR: u64 = 10;
const WRAP_NATIVE_CLAIMS: u64 = 7;
const OUTCOME: u32 = 1;
const CUBIC_PAYOUT_SCALE: u64 = 11;
const CURVED_RESULT_NUMERATOR: i128 = 3;
const CURVED_RESULT_DENOMINATOR: u64 = 2;
const ACTOR_FUNDED_BALANCE: u64 = 1_000;
const RENT_CREDIT: Pubkey = Pubkey::new_from_array([0x65; 32]);
const GRAPH_ID: [u8; 32] = [0x7c; 32];
const EXPOSURE_ID: [u8; 32] = [0x7a; 32];
const HOLDER_TOKEN: Pubkey = Pubkey::new_from_array([0x78; 32]);
const SLEEPER_TOKEN: Pubkey = Pubkey::new_from_array([0xc1; 32]);
const SLEEPER_SHARDS: u64 = 40;
/// Representation width the exterior runs at, inside the settleable bound.
pub const WIDTH: usize = 4;

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
    /// Program receiving the instruction.
    pub program: Pubkey,
    /// Exact caller wrapper bytes.
    pub data: Vec<u8>,
    /// Exact ordered account frame, caller program first.
    pub metas: Vec<Meta>,
    /// Complete protocol poststate required after this action commits.
    pub expected: ExpectedPoststate,
}

/// Exact four-ledger poststate for one exterior action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpectedPoststate {
    /// Outstanding Token-2022 shard supply.
    pub shard_mint_supply: u64,
    /// Actor's shard-token balance.
    pub holder_token_amount: u64,
    /// Independent sleeping holder's shard-token balance.
    pub sleeper_token_amount: u64,
    /// Actor's native Claims balance at the represented coordinate.
    pub actor_native_claims: u64,
    /// Capability reserve's native Claims balance.
    pub reserve_native_claims: u64,
}

pub(crate) const WRAP_EXPECTED: ExpectedPoststate = ExpectedPoststate {
    shard_mint_supply: WRAP_NATIVE_CLAIMS * DENOMINATOR,
    holder_token_amount: WRAP_NATIVE_CLAIMS * DENOMINATOR,
    sleeper_token_amount: 0,
    actor_native_claims: ACTOR_FUNDED_BALANCE - WRAP_NATIVE_CLAIMS,
    reserve_native_claims: WRAP_NATIVE_CLAIMS,
};

pub(crate) const SLEEPER_TRANSFER_EXPECTED: ExpectedPoststate = ExpectedPoststate {
    shard_mint_supply: WRAP_NATIVE_CLAIMS * DENOMINATOR,
    holder_token_amount: WRAP_NATIVE_CLAIMS * DENOMINATOR - SLEEPER_SHARDS,
    sleeper_token_amount: SLEEPER_SHARDS,
    actor_native_claims: ACTOR_FUNDED_BALANCE - WRAP_NATIVE_CLAIMS,
    reserve_native_claims: WRAP_NATIVE_CLAIMS,
};

pub(crate) const WHOLE_UNWRAP_EXPECTED: ExpectedPoststate = ExpectedPoststate {
    shard_mint_supply: SLEEPER_SHARDS,
    holder_token_amount: 0,
    sleeper_token_amount: SLEEPER_SHARDS,
    actor_native_claims: ACTOR_FUNDED_BALANCE - SLEEPER_SHARDS / DENOMINATOR,
    reserve_native_claims: SLEEPER_SHARDS / DENOMINATOR,
};

/// Canonical action order and exact poststates independently re-read by `verify`.
pub(crate) const EXPECTED_ACTIONS: [(&str, ExpectedPoststate); 3] = [
    ("wrap", WRAP_EXPECTED),
    ("token-2022-transfer-to-sleeper", SLEEPER_TRANSFER_EXPECTED),
    ("whole-unwrap-actor-remainder", WHOLE_UNWRAP_EXPECTED),
];

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
    /// Independent sleeping holder's Token-2022 account.
    pub sleeper_token: Pubkey,
    /// Actor Position, observed for native Claims.
    pub actor_position: Pubkey,
    /// Reserve Position, observed for locked Claims.
    pub reserve_position: Pubkey,
    /// Representation coordinate driven by this exterior.
    pub representation_coordinate: usize,
    /// Activated release-set content identity shared by every phase.
    pub release_set: [u8; 32],
    /// Canonical Realm record content identity.
    pub realm: [u8; 32],
    /// Product record content identity.
    pub product: [u8; 32],
    /// ProductBasisV3 record content identity.
    pub product_basis: [u8; 32],
    /// Fractional terms record content identity.
    pub terms: [u8; 32],
    /// Claims aggregate carried into compaction.
    pub aggregate: Pubkey,
    /// Core Market carried through terminal resolution.
    pub market: Pubkey,
    /// Capability root carried into compaction.
    pub root: Pubkey,
    /// Sleeping holder identity carried into compaction.
    pub sleeper_owner: Pubkey,
    /// Exact outstanding shard atoms carried into compaction.
    pub sleeper_shards: u64,
}

fn selection_config_digest(terms: &NarrowRecordV2) -> [u8; 32] {
    let decoded = FractionalExposureTermsV2::decode(
        &terms.bytes,
        FractionalExposureTermsAdmissionV2 {
            selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            selected_terms_id: terms.digest,
            finalized_terms_id: terms.digest,
            recomputed_terms_digest: terms.digest,
            finalized_terms_digest: terms.digest,
            record_authenticated: true,
        },
    )
    .expect("finalized Fractional terms");
    let mut config = [0_u8; FRACTIONAL_SELECTION_CONFIG_BYTES_V1];
    encode_fractional_selection_config_v1(
        fractional_selection_config_from_terms_v1(decoded),
        &mut config,
    )
    .expect("Fractional selection config");
    hash(&config).to_bytes()
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
    /// Custody program.
    pub custody: &'a [u8],
    /// Test caller standing in for Trading.
    pub caller: &'a [u8],
}

fn activation_cache(elves: &Elves<'_>) -> ([u8; 32], Vec<u8>) {
    let core = release(CORE, 0x31, elves.core);
    let claims = release(CLAIMS, 0x32, elves.claims);
    let trading = release(CALLER, 0x33, elves.caller);
    let custody = release(CUSTODY, 0x34, elves.custody);
    let set = ExecutionReleaseSetV1::new(
        binding(core),
        binding(claims),
        binding(trading),
        binding(claims),
        binding(custody),
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
        (ExecutionRoleV1::Custody, custody),
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
pub fn stage(elves: &Elves<'_>, actor: Pubkey, sleeper_owner: Pubkey) -> Staged {
    let (release_set, cache) = activation_cache(elves);
    let cache_key =
        Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &release_set], &REGISTRY).0;

    let adapter = PRODUCTION_ADAPTER_RELEASES
        .get(1)
        .copied()
        .expect("Token-2022 production adapter");
    let realm = finalized(
        REGISTRY,
        REALM_SCHEMA_RELEASE_ID_V1,
        RealmV1::new(RealmV1Input {
            token_program: TOKEN_2022_PROGRAM_ID,
            collateral_mint: COLLATERAL_MINT.to_bytes(),
            collateral_adapter_release_id: hash(&adapter.to_bytes()).to_bytes(),
            mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
            freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
        })
        .expect("canonical Realm")
        .to_bytes()
        .to_vec(),
    );
    let probe = compile(
        release_set,
        realm.digest,
        actor,
        Pubkey::new_from_array([0xef; 32]),
    );
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
        TokenBehaviorSelectionV2::new(realm.digest, release_set)
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
        ContentId::new(dclutch_claims::fractional::FRACTIONAL_CAPABILITY_KIND_ID_V1)
            .expect("kind"),
        ContentId::new([0x83; 32]).expect("capability release"),
        ContentId::new(selection_config_digest(&terms)).expect("config"),
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
    let shared = compile(release_set, realm.digest, actor, root);
    assert_eq!(shared.core_market, core_market, "Market must not move");

    let state = FractionalRootV1::new(FractionalRootInputV1 {
        bump,
        terms: terms.digest,
        market: core_market.to_bytes(),
        rent_beneficiary: sleeper_owner.to_bytes(),
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
            key: SLEEPER_TOKEN,
            owner: token_program(),
            data: token_account_bytes(shard_mint, sleeper_owner, 0),
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
        &realm,
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
        (
            "whole-unwrap-actor-remainder",
            FractionalExposureActionV2::WholeUnwrap,
        ),
    ] {
        let (source, destination, quantity) = match action {
            FractionalExposureActionV2::Wrap => {
                ([0; 32], HOLDER_TOKEN.to_bytes(), WRAP_NATIVE_CLAIMS)
            }
            _ => (
                HOLDER_TOKEN.to_bytes(),
                [0; 32],
                WRAP_NATIVE_CLAIMS * DENOMINATOR - SLEEPER_SHARDS,
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
        actions.push(StagedAction {
            name,
            program: CALLER,
            data,
            metas,
            expected: match action {
                FractionalExposureActionV2::Wrap => WRAP_EXPECTED,
                FractionalExposureActionV2::WholeUnwrap => WHOLE_UNWRAP_EXPECTED,
                _ => unreachable!("the exterior stages only whole-claim actions"),
            },
        });
        if matches!(action, FractionalExposureActionV2::Wrap) {
            let mut transfer = Vec::with_capacity(10);
            // SPL Token-2022 TransferChecked(amount, decimals=0).
            transfer.push(12);
            transfer.extend_from_slice(&SLEEPER_SHARDS.to_le_bytes());
            transfer.push(0);
            actions.push(StagedAction {
                name: "token-2022-transfer-to-sleeper",
                program: token_program(),
                data: transfer,
                metas: vec![
                    Meta {
                        key: HOLDER_TOKEN,
                        signer: false,
                        writable: true,
                    },
                    Meta {
                        key: shard_mint,
                        signer: false,
                        writable: false,
                    },
                    Meta {
                        key: SLEEPER_TOKEN,
                        signer: false,
                        writable: true,
                    },
                    Meta {
                        key: actor,
                        signer: true,
                        writable: false,
                    },
                ],
                expected: SLEEPER_TRANSFER_EXPECTED,
            });
        }
    }

    Staged {
        accounts,
        actions,
        shard_mint,
        holder_token: HOLDER_TOKEN,
        sleeper_token: SLEEPER_TOKEN,
        actor_position: shared.actor_position.account,
        reserve_position: shared.reserve_position.account,
        representation_coordinate: OUTCOME as usize,
        release_set,
        realm: realm.digest,
        product: shared.product.digest,
        product_basis: shared.linked_basis.digest,
        terms: terms.digest,
        aggregate: shared.claims_market,
        market: shared.core_market,
        root,
        sleeper_owner,
        sleeper_shards: SLEEPER_SHARDS,
    }
}

fn compile(
    release_set: [u8; 32],
    realm_id: [u8; 32],
    actor: Pubkey,
    reserve: Pubkey,
) -> NarrowFixtureV2 {
    let knots = [0_i128, 0, 0, 0, 3, 3, 3, 3];
    let failure_payouts = [0_u64, 0, 0, CUBIC_PAYOUT_SCALE];
    let price_gate = curved_price_gate_certificate();
    compile_narrow_fixture_v3(
        NarrowFixtureInputV2 {
            outcome_count: WIDTH,
            registry_program: REGISTRY,
            core_program: CORE,
            claims_program: CLAIMS,
            release_set,
            realm_id,
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
        },
        NarrowBasisInputV3::SplineDegree2To3(NarrowSplineBasisInputV3 {
            degree: 3,
            interior_multiplicity: false,
            payout_scale: CUBIC_PAYOUT_SCALE,
            knot_denominator: 1,
            knots: &knots,
            failure_payouts: &failure_payouts,
            price_gate_certificate: &price_gate,
        }),
    )
    .expect("narrow fixture at the exterior width")
}

/// Canonical one-atom DCLTPGT1 witness for the cubic midpoint partition.
fn curved_price_gate_certificate() -> [u8; PRICE_GATE_REQUEST_BYTES_V1] {
    let mut certificate = [0_u8; PRICE_GATE_REQUEST_BYTES_V1];
    certificate[PRICE_GATE_MAGIC_OFFSET_V1..PRICE_GATE_MAGIC_OFFSET_V1 + 8]
        .copy_from_slice(&PRICE_GATE_MAGIC_V1);
    certificate[PRICE_GATE_VERSION_OFFSET_V1..PRICE_GATE_VERSION_OFFSET_V1 + 2]
        .copy_from_slice(&PRICE_GATE_SCHEMA_VERSION_V1.to_le_bytes());
    certificate[PRICE_GATE_PROFILE_OFFSET_V1..PRICE_GATE_PROFILE_OFFSET_V1 + 2]
        .copy_from_slice(&PRICE_GATE_PROFILE_V1.to_le_bytes());
    certificate[PRICE_GATE_SCALE_OFFSET_V1..PRICE_GATE_SCALE_OFFSET_V1 + 4].copy_from_slice(
        &u32::try_from(CUBIC_PAYOUT_SCALE)
            .expect("price-gate scale")
            .to_le_bytes(),
    );
    certificate[PRICE_GATE_MASS_OFFSET_V1..PRICE_GATE_MASS_OFFSET_V1 + 8]
        .copy_from_slice(&1_u64.to_le_bytes());
    certificate[PRICE_GATE_DEGREE_OFFSET_V1] = 3;
    certificate[PRICE_GATE_WIDTH_OFFSET_V1] = u8::try_from(WIDTH).expect("price-gate width");
    certificate[PRICE_GATE_ATOM_COUNT_OFFSET_V1] = 1;
    for (claim, payout) in [1_u64, 4, 4, 2].iter().enumerate() {
        let offset = PRICE_GATE_PRICES_OFFSET_V1 + claim * 8;
        certificate[offset..offset + 8].copy_from_slice(&payout.to_le_bytes());
    }
    certificate[PRICE_GATE_WEIGHTS_OFFSET_V1..PRICE_GATE_WEIGHTS_OFFSET_V1 + 8]
        .copy_from_slice(&1_u64.to_le_bytes());
    certificate[PRICE_GATE_NUMERATORS_OFFSET_V1..PRICE_GATE_NUMERATORS_OFFSET_V1 + 8]
        .copy_from_slice(
            &i64::try_from(CURVED_RESULT_NUMERATOR)
                .expect("price-gate coordinate")
                .to_le_bytes(),
        );
    certificate[PRICE_GATE_DENOMINATORS_OFFSET_V1..PRICE_GATE_DENOMINATORS_OFFSET_V1 + 4]
        .copy_from_slice(
            &u32::try_from(CURVED_RESULT_DENOMINATOR)
                .expect("price-gate denominator")
                .to_le_bytes(),
        );
    certificate
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
