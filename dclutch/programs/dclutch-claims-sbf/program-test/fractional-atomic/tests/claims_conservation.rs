//! Real-ELF evidence for the Claims-owned conservation route, `DCLCNS01`.
//!
//! # What this campaign found
//!
//! **The split/merge user act cannot execute, and no frame can make it.** The
//! route reads every one of its three subject accounts with TWO decoders that
//! belong to DIFFERENT account families, and no byte string satisfies both.
//!
//! - `LiabilityBasisMarketViewV2` / `LiabilityBasisPositionViewV2` — the LBV2
//!   family. Aggregate magic `DCLLBM02`, header 256, ONE `u64` supplies vector,
//!   and **no Hoard scalar at all**; Position magic `DCLLBP02`, header 128, one
//!   balances vector. This is what `founding_v5` creates, at
//!   `b"dclutch:lbv2:market"`, and what `ProtocolPositionSeedsV2` addresses.
//! - `dclutch_product::economic_slice::{market_hoard, market_supply,
//!   position_native, position_revision, execute_basket}` — the economic-slice
//!   family. Market magic `DCLTEMK2`, header 144, THREE vectors, a Hoard scalar
//!   at offset 32; Position magic `DCLTEPS2`, header 96, two vectors. This is
//!   what `programs/dclutch-claims-sbf/src/lib.rs`'s `initialize_market`
//!   creates, at `b"dclutch:claims-aggregate:v1"`.
//!
//! `claims_conservation_v1.rs` decodes the aggregate as LBV2 at its line 358
//! and then hands the SAME borrow to `market_hoard` at line 371; it derives the
//! actor's and the escrow's Positions with `ProtocolPositionSeedsV2`, decodes
//! them with `PositionViewV2`, and mutates them with `execute_basket`. Whatever
//! is supplied, one of the two readers refuses first.
//!
//! # The two-sided proof, on the ELF that is committed
//!
//! One frame, twenty-nine accounts, one request. The ONLY thing that differs
//! between the two campaigns below is the AGGREGATE ACCOUNT'S BYTES, planted at
//! the same address under the same owner:
//!
//! | aggregate bytes | first reader to refuse | observed |
//! |---|---|---|
//! | LBV2 `DCLLBM02` (what founding writes) | `market_hoard`, line 371 | `ClaimsSbfError::Economic` |
//! | economic slice `DCLTEMK2` | `MarketViewV2::decode`, line 358 | `ClaimsSbfError::Identity` |
//!
//! Two different refusals from one route over one frame is the shape of a
//! disagreement between two authorities, not of a fixture that is merely wrong:
//! a fixture can be corrected, and there is no third aggregate.
//!
//! # What the repair needs, which is why this lane did not attempt one
//!
//! The identity half CANNOT move to the economic-slice family: the record join
//! `authenticate_runtime_product_basis_core_with_rent_v3` needs `basis_id`,
//! `realm_id`, `custody_context` and `generation`, and an economic-slice market
//! header carries none of the four. The economics half cannot move to LBV2
//! either: LBV2 has no Hoard scalar to hold outstanding principal, and the tree
//! has no LBV2 complete-set executor — `signed_delta_v3` and `affine_batch_v2`
//! each open-code a private `apply_coordinate`, and nothing in the tree mints or
//! burns a uniform vector against a live LBV2 aggregate. Closing this is a
//! ruling about where an LBV2 Market's outstanding principal lives, not a
//! substitution, so this campaign names the wall and stands as the harness the
//! repair turns green.
//!
//! # The fixture join
//!
//! This is also the join the refunding work was owed: `affine-batch` had the
//! LBV2 record set and no Custody, `fractional-atomic` had Token-2022, a real
//! Custody vault and a HoardPrincipal compartment and no founded LBV2
//! aggregate. Here they are one world — a width-4 REFUNDING Market (payout
//! scale `basis_width - 1`, which is what `categorical_refunds_on_failure_v3`
//! reads), its aggregate carrying a complete set at every coordinate, the
//! founder holding the three ordinary coordinates, the Market's own derived
//! failure escrow holding the failure column, a Token-2022 collateral mint, the
//! Custody-derived HoardPrincipal vault funded, and a stranger with atoms to
//! spend. Everything past the wall — the Custody replay cursor's state and the
//! delegated allowance — is planted at the shape the frame requires and is NOT
//! exercised, because the route refuses four checks earlier; that is stated
//! rather than dressed up.

use std::{env, fs, path::PathBuf};

use dclutch_claims::conservation::{
    CLAIMS_CONSERVATION_REQUEST_BYTES_V1, ClaimsConservationDirectionV1,
    ClaimsConservationRequestV1,
};
use dclutch_claims_sbf::ClaimsSbfError;
use dclutch_claims::{
    liability_basis_state_v2::{LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2},
    protocol_position_v2::ProtocolPositionClaimsCapabilitySeedsV2,
};
use dclutch_custody::{
    CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CompartmentV1, CustodyAuthoritySeedsV1,
    CustodyReplaySeedsV1, CustodyVaultSeedsV1,
};
use dclutch_fractional_atomic_program_test::{
    campaign_support::{
        ReleaseSetInputV1, activation_cache, add_account, add_upgradeable_program,
        collateral_mint_bytes, finalized, programdata_address, token_account_bytes_for,
        token_program_id,
    },
    narrow_fixture::{
        NarrowBasisInputV3, NarrowFixtureInputV2, NarrowFixtureV2, compile_narrow_fixture_v3,
        compile_narrow_position_v2, put_narrow_market_supplies_v2,
    },
};
use dclutch_program_test_evidence::TransactionEvidence;
use dclutch_market::realm::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_custody::token_svm::{PRODUCTION_ADAPTER_RELEASES, TOKEN_2022_PROGRAM_ID};
use solana_program::{
    clock::Clock,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::{signature::Keypair, signer::Signer};
use solana_sdk_ids::system_program;
use solana_transaction::Transaction;

// ---------------------------------------------------------------------------
// Identities
// ---------------------------------------------------------------------------

const CLAIMS_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa1; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa2; 32]);
const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa3; 32]);
const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa4; 32]);
const COLLATERAL_MINT: Pubkey = Pubkey::new_from_array([0x74; 32]);
const STRANGER_COLLATERAL: Pubkey = Pubkey::new_from_array([0xc2; 32]);
const REFUND_WALLET: [u8; 32] = [0x5f; 32];

/// Runtime complete-set width. Three ordinary coordinates and one failure
/// coordinate: the narrowest width a categorical record may be founded
/// refunding at is three (`CATEGORICAL_REFUND_MINIMUM_WIDTH_V3`), and four
/// leaves the failure coordinate unambiguously distinguishable from the last
/// ordinary one in every assertion below.
const CLAIM_COUNT: u32 = 4;
/// Complete sets the founding issued.
const FOUNDED_SETS: u64 = 10;
/// Complete sets the stranger's split would create.
const SPLIT_SETS: u64 = 5;
/// Collateral atoms the Hoard already holds against `FOUNDED_SETS`.
const HOARD_ATOMS: u64 = 30;
/// Collateral atoms the stranger holds before the split.
const STRANGER_ATOMS: u64 = 100;
const COLLATERAL_DECIMALS: u8 = 6;
const GENERATION: u64 = 41;
const CUSTODY_CONTEXT: [u8; 32] = [0x62; 32];
const POSITION_REVISION: u64 = 0;

fn stranger_keypair() -> Keypair {
    Keypair::new_from_array([0x21; 32])
}

// ---------------------------------------------------------------------------
// Artifacts
// ---------------------------------------------------------------------------

struct Artifacts {
    claims: Vec<u8>,
    registry: Vec<u8>,
    core: Vec<u8>,
    custody: Vec<u8>,
    token: Vec<u8>,
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
        registry: read("dclutch_registry_sbf.so"),
        core: read("dclutch_core_sbf.so"),
        custody: read("dclutch_custody_sbf.so"),
        token: read("spl_token_2022.so"),
    }
}

// ---------------------------------------------------------------------------
// The world
// ---------------------------------------------------------------------------

/// Which aggregate BYTES this campaign plants at the LBV2 aggregate address.
///
/// The address, the owner, the request and the other twenty-eight accounts are
/// identical either way. This enum is the whole independent variable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AggregateBodyV1 {
    /// What `founding_v5` writes: `DCLLBM02`, one supplies vector, no Hoard.
    LiabilityBasisV2,
    /// What `lib.rs`'s `initialize_market` writes: `DCLTEMK2`, three vectors,
    /// a Hoard scalar. Planted at the LBV2 address on purpose — the route's
    /// privilege pass compares the account's KEY to the request and never asks
    /// which family the bytes belong to, so this reaches the same reader.
    EconomicSlice,
}

struct World {
    shared: NarrowFixtureV2,
    activation_cache: Pubkey,
    release_set: [u8; 32],
    realm_id: [u8; 32],
    realm_raw: Pubkey,
    realm_staging: Pubkey,
    escrow_position: Pubkey,
    hoard: Pubkey,
    custody_authority: Pubkey,
    custody_replay: Pubkey,
}

/// The founded refunding Market, the Custody world, and the Token-2022 mint.
fn world(body: AggregateBodyV1) -> (ProgramTest, World) {
    let artifacts = artifacts();
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    for (name, program, elf) in [
        (
            "dclutch_claims_sbf",
            CLAIMS_PROGRAM_ID,
            artifacts.claims.as_slice(),
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
            "dclutch_custody_sbf",
            CUSTODY_PROGRAM_ID,
            artifacts.custody.as_slice(),
        ),
    ] {
        add_upgradeable_program(&mut test, name, program, elf);
    }
    test.add_program("spl_token_2022", token_program_id(), None);
    let _ = &artifacts.token;

    // The Trading role binds the Registry program. This route never reads the
    // Trading binding -- its actor signs for their own Position and for nothing
    // else -- and binding it to a program this world already loads is honest
    // about that rather than inventing a caller nothing invokes.
    let (release_set, cache_bytes) = activation_cache(&ReleaseSetInputV1 {
        core: (CORE_PROGRAM_ID, artifacts.core.as_slice()),
        claims: (CLAIMS_PROGRAM_ID, artifacts.claims.as_slice()),
        trading: (REGISTRY_PROGRAM_ID, artifacts.registry.as_slice()),
        custody: Some((CUSTODY_PROGRAM_ID, artifacts.custody.as_slice())),
    });
    let activation_cache_key = Pubkey::find_program_address(
        &[
            dclutch_registry::ACTIVATION_PDA_DOMAIN_V1,
            &release_set,
        ],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    add_account(
        &mut test,
        activation_cache_key,
        REGISTRY_PROGRAM_ID,
        cache_bytes,
    );

    let adapter = PRODUCTION_ADAPTER_RELEASES
        .get(1)
        .copied()
        .expect("Token-2022 production adapter");
    let realm_bytes = RealmV1::new(RealmV1Input {
        token_program: TOKEN_2022_PROGRAM_ID,
        collateral_mint: COLLATERAL_MINT.to_bytes(),
        collateral_adapter_release_id: hash(&adapter.to_bytes()).to_bytes(),
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .expect("canonical Realm")
    .to_bytes()
    .to_vec();
    let realm_record = finalized(REGISTRY_PROGRAM_ID, REALM_SCHEMA_RELEASE_ID_V1, realm_bytes);

    let stranger = stranger_keypair().pubkey();
    let founder = Pubkey::new_from_array([0x31; 32]);
    let shared = compile_narrow_fixture_v3(
        NarrowFixtureInputV2 {
            outcome_count: usize::try_from(CLAIM_COUNT).expect("width"),
            registry_program: REGISTRY_PROGRAM_ID,
            core_program: CORE_PROGRAM_ID,
            claims_program: CLAIMS_PROGRAM_ID,
            release_set,
            realm_id: realm_record.digest,
            custody_context: CUSTODY_CONTEXT,
            generation: GENERATION,
            // The STRANGER is the actor: the split under test is a stranger's,
            // not the founder's, which is the case the failure walk needs.
            actor_owner: stranger,
            reserve_owner: founder,
            funded_coordinate: 0,
            funded_balance: 0,
            position_revision: POSITION_REVISION,
            reserve_balance: 0,
            terminal: None,
            rent_beneficiary: Pubkey::new_from_array(REFUND_WALLET),
            graph_id: [0x34; 32],
            exposure_id: [0x35; 32],
        },
        NarrowBasisInputV3::CategoricalRefunding,
    )
    .expect("refunding narrow fixture");
    assert_eq!(
        shared.payout_scale,
        u64::from(CLAIM_COUNT - 1),
        "a refunding categorical basis pays `basis_width - 1`, which is what \
         `categorical_refunds_on_failure_v3` reads the shape off",
    );

    // A FOUNDED Market's aggregate holds a complete set at every coordinate.
    // The compiler funds one coordinate, which no founding ever leaves behind.
    let failure_selector = CLAIM_COUNT - 1;
    let supplies = vec![FOUNDED_SETS; usize::try_from(CLAIM_COUNT).expect("width")];
    let mut claims_market_bytes = shared.claims_market_bytes.clone();
    put_narrow_market_supplies_v2(&mut claims_market_bytes, &supplies)
        .expect("a founded aggregate carries one complete set at every coordinate");

    // The founder holds the ordinary coordinates and no failure claim; the
    // Market's own derived escrow holds the failure column and nothing else.
    // Together they are exactly one complete set -- which is founding v6's
    // `refunding_founding_vectors_v1` seen from the outside.
    let mut founder_balances = vec![FOUNDED_SETS; usize::try_from(CLAIM_COUNT).expect("width")];
    *founder_balances
        .get_mut(usize::try_from(failure_selector).expect("selector"))
        .expect("failure coordinate") = 0;
    let founder_position = compile_narrow_position_v2(
        CLAIMS_PROGRAM_ID,
        shared.claims_market,
        founder,
        shared.semantic_basis_id,
        &founder_balances,
        POSITION_REVISION,
    )
    .expect("founder Position");

    let escrow_owner = escrow_owner_v1(shared.core_market, failure_selector);
    let mut escrow_balances = vec![0_u64; usize::try_from(CLAIM_COUNT).expect("width")];
    *escrow_balances
        .get_mut(usize::try_from(failure_selector).expect("selector"))
        .expect("failure coordinate") = FOUNDED_SETS;
    let escrow_position_body = compile_narrow_position_v2(
        CLAIMS_PROGRAM_ID,
        shared.claims_market,
        escrow_owner,
        shared.semantic_basis_id,
        &escrow_balances,
        POSITION_REVISION,
    )
    .expect("failure escrow Position");

    for record in [
        &shared.product,
        &shared.result_domain,
        &shared.portfolio,
        &shared.linked_basis,
        &realm_record,
    ] {
        add_account(&mut test, record.raw, record.owner, record.bytes.clone());
        add_account(&mut test, record.staging, system_program::ID, Vec::new());
    }
    add_account(
        &mut test,
        shared.core_market,
        CORE_PROGRAM_ID,
        shared.core_state.clone(),
    );
    let aggregate_bytes = match body {
        AggregateBodyV1::LiabilityBasisV2 => claims_market_bytes,
        AggregateBodyV1::EconomicSlice => economic_slice_aggregate_bytes(&shared, release_set),
    };
    add_account(
        &mut test,
        shared.claims_market,
        CLAIMS_PROGRAM_ID,
        aggregate_bytes,
    );
    for position in [
        &shared.actor_position,
        &founder_position,
        &escrow_position_body,
    ] {
        add_account(
            &mut test,
            position.account,
            CLAIMS_PROGRAM_ID,
            position.bytes.clone(),
        );
    }

    // The Custody world, at its own derived coordinates.
    let custody_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(shared.core_market.to_bytes(), release_set).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let hoard = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            shared.core_market.to_bytes(),
            release_set,
            CUSTODY_CONTEXT,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let custody_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(
            shared.core_market.to_bytes(),
            release_set,
            CallerRoleV1::Claims,
            CUSTODY_CONTEXT,
        )
        .as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    add_account(
        &mut test,
        COLLATERAL_MINT,
        token_program_id(),
        collateral_mint_bytes(HOARD_ATOMS + STRANGER_ATOMS, COLLATERAL_DECIMALS),
    );
    add_account(
        &mut test,
        hoard,
        token_program_id(),
        token_account_bytes_for(COLLATERAL_MINT, custody_authority, HOARD_ATOMS),
    );
    add_account(
        &mut test,
        STRANGER_COLLATERAL,
        token_program_id(),
        token_account_bytes_for(COLLATERAL_MINT, stranger, STRANGER_ATOMS),
    );
    // PLANTED AND NOT EXERCISED, and said so rather than dressed up: the route
    // refuses at the aggregate's second reader, four checks before the replay
    // cursor's body is decoded. The frame needs it writable and at its derived
    // address; what it holds is the repair's business.
    add_account(
        &mut test,
        custody_replay,
        CUSTODY_PROGRAM_ID,
        vec![0_u8; CUSTODY_REPLAY_BYTES_V1],
    );
    add_account(&mut test, custody_authority, system_program::ID, Vec::new());
    test.add_account(
        stranger,
        solana_account::Account {
            lamports: 10_000_000_000,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    (
        test,
        World {
            shared,
            activation_cache: activation_cache_key,
            release_set,
            realm_id: realm_record.digest,
            realm_raw: realm_record.raw,
            realm_staging: realm_record.staging,
            escrow_position: escrow_position_body.account,
            hoard,
            custody_authority,
            custody_replay,
        },
    )
}

/// The Market's own failure-escrow owner, derived exactly as the program does.
///
/// `FailureEscrowIdentityV1::derive` is crate-private, so this restates its two
/// steps -- `refunding_failure_index` and the claims-capability seeds -- from
/// the same public seed helper the program uses. If they ever disagree the
/// route refuses `0x5010 FailureEscrow`, which is a louder failure than a
/// silent mismatch would be.
fn escrow_owner_v1(core_market: Pubkey, failure_selector: u32) -> Pubkey {
    Pubkey::find_program_address(
        &ProtocolPositionClaimsCapabilitySeedsV2::new(core_market.to_bytes(), failure_selector)
            .expect("claims-capability escrow seeds")
            .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0
}

/// A canonical ECONOMIC-SLICE aggregate for the same Market, at the same width.
///
/// This is the other family's answer to the same question, built by that
/// family's own initializer so it cannot be accused of being a hand-rolled
/// straw man.
fn economic_slice_aggregate_bytes(shared: &NarrowFixtureV2, release_set: [u8; 32]) -> Vec<u8> {
    use dclutch_product::economic_slice::{
        MARKET_HEADER_BYTES, Phase, SCALAR_BYTES, initialize_market,
    };
    let count = usize::try_from(CLAIM_COUNT).expect("width");
    let width = MARKET_HEADER_BYTES + count * 3 * SCALAR_BYTES;
    let mut bytes = vec![0_u8; width];
    initialize_market(
        &mut bytes,
        shared.core_market.to_bytes(),
        release_set,
        REGISTRY_PROGRAM_ID.to_bytes(),
        CLAIM_COUNT,
        Phase::Open,
        FOUNDED_SETS,
    )
    .expect("canonical economic-slice aggregate");
    bytes
}

// ---------------------------------------------------------------------------
// The request and its frame
// ---------------------------------------------------------------------------

/// One stranger's split of `SPLIT_SETS` complete sets on the founded Market.
fn split_request(world: &World) -> ClaimsConservationRequestV1 {
    let collateral_atoms = SPLIT_SETS * world.shared.payout_scale;
    let request = ClaimsConservationRequestV1 {
        direction: ClaimsConservationDirectionV1::Split,
        realm: world.realm_id,
        market: world.shared.core_market.to_bytes(),
        release_set: world.release_set,
        custody_context: CUSTODY_CONTEXT,
        aggregate: world.shared.claims_market.to_bytes(),
        position: world.shared.actor_position.account.to_bytes(),
        owner: stranger_keypair().pubkey().to_bytes(),
        external_collateral: STRANGER_COLLATERAL.to_bytes(),
        hoard_vault: world.hoard.to_bytes(),
        mint: COLLATERAL_MINT.to_bytes(),
        token_program: token_program_id().to_bytes(),
        claims_program: CLAIMS_PROGRAM_ID.to_bytes(),
        product_record_digest: world.shared.product.digest,
        linked_basis_record_digest: world.shared.linked_basis.digest,
        semantic_basis_id: world.shared.semantic_basis_id,
        generation: GENERATION,
        quantity: SPLIT_SETS,
        basis_scale: world.shared.payout_scale,
        collateral_atoms,
        expected_market_revision: 0,
        expected_position_revision: POSITION_REVISION,
        expected_custody_revision: 1,
        pre_external_amount: STRANGER_ATOMS,
        post_external_amount: STRANGER_ATOMS - collateral_atoms,
        pre_hoard_amount: HOARD_ATOMS,
        post_hoard_amount: HOARD_ATOMS + collateral_atoms,
        claim_count: CLAIM_COUNT,
    };
    request
        .validate()
        .expect("the split this campaign submits is a conserving one");
    request
}

/// The exact twenty-nine-account conservation frame, in the route's own order.
fn split_instruction(world: &World, request: ClaimsConservationRequestV1) -> Instruction {
    let bytes = request.to_bytes().expect("canonical request bytes");
    let parent_digest = hash(&bytes).to_bytes();
    let custody = request
        .custody_request(parent_digest)
        .expect("derived Custody request");
    let delegated = request
        .delegated_custody_request(parent_digest, world.custody_authority.to_bytes())
        .expect("delegated Custody request")
        .encode()
        .expect("delegated Custody wire");
    let caller_authority = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::from_bytes(
            custody.release_set,
            custody.market,
            ExecutionRoleV1::Claims,
            custody.context,
            hash(&delegated).to_bytes(),
        )
        .expect("claims-role caller seeds")
        .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let shared = &world.shared;
    let accounts = vec![
        AccountMeta::new_readonly(stranger_keypair().pubkey(), true),
        AccountMeta::new(shared.claims_market, false),
        AccountMeta::new(shared.actor_position.account, false),
        AccountMeta::new(world.escrow_position, false),
        AccountMeta::new_readonly(shared.core_market, false),
        AccountMeta::new_readonly(shared.linked_basis.raw, false),
        AccountMeta::new_readonly(shared.linked_basis.staging, false),
        AccountMeta::new_readonly(shared.product.raw, false),
        AccountMeta::new_readonly(shared.product.staging, false),
        AccountMeta::new_readonly(shared.result_domain.raw, false),
        AccountMeta::new_readonly(shared.result_domain.staging, false),
        AccountMeta::new_readonly(shared.portfolio.raw, false),
        AccountMeta::new_readonly(shared.portfolio.staging, false),
        AccountMeta::new_readonly(world.activation_cache, false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(CLAIMS_PROGRAM_ID), false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(CORE_PROGRAM_ID), false),
        AccountMeta::new_readonly(caller_authority, false),
        AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
        AccountMeta::new(world.custody_replay, false),
        AccountMeta::new(world.hoard, false),
        AccountMeta::new(STRANGER_COLLATERAL, false),
        AccountMeta::new_readonly(COLLATERAL_MINT, false),
        AccountMeta::new_readonly(token_program_id(), false),
        AccountMeta::new_readonly(world.custody_authority, false),
        AccountMeta::new_readonly(world.realm_raw, false),
        AccountMeta::new_readonly(world.realm_staging, false),
    ];
    assert_eq!(
        accounts.len(),
        dclutch_claims_sbf::claims_conservation_v1::CLAIMS_CONSERVATION_ACCOUNT_COUNT_V1,
        "the frame is the route's own declared width, read off the route",
    );
    assert_eq!(bytes.len(), CLAIMS_CONSERVATION_REQUEST_BYTES_V1);
    Instruction {
        program_id: CLAIMS_PROGRAM_ID,
        accounts,
        data: bytes.to_vec(),
    }
}

// ---------------------------------------------------------------------------
// Submission
// ---------------------------------------------------------------------------

struct Outcome {
    accepted: bool,
    units: u64,
    refusal: Option<u32>,
    logs: Vec<String>,
}

async fn submit(
    context: &mut ProgramTestContext,
    label: &str,
    instruction: Instruction,
) -> Outcome {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let payer = context.payer.insecure_clone();
    let stranger = stranger_keypair();
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer, &stranger],
        blockhash,
    );
    let signature = transaction
        .signatures
        .first()
        .map(ToString::to_string)
        .expect("a submitted transaction carries its own signature");
    let wire_bytes = 1_usize + transaction.signatures.len() * 64 + transaction.message_data().len();
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("transaction processing");
    let units = processed
        .metadata
        .clone()
        .map(|metadata| metadata.compute_units_consumed)
        .unwrap_or_default();
    let logs = processed
        .metadata
        .clone()
        .map(|metadata| metadata.log_messages)
        .unwrap_or_default();
    let failure = processed
        .result
        .clone()
        .err()
        .map(|error| format!("{error:?}"));
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
    let refusal = match &processed.result {
        Err(solana_sdk::transaction::TransactionError::InstructionError(
            _,
            solana_sdk::instruction::InstructionError::Custom(code),
        )) => Some(*code),
        _ => None,
    };
    Outcome {
        accepted: processed.result.is_ok(),
        units,
        refusal,
        logs,
    }
}

async fn run(body: AggregateBodyV1, label: &str) -> Outcome {
    let (test, world) = world(body);
    let mut context = test.start_with_context().await;
    let request = split_request(&world);
    let instruction = split_instruction(&world, request);
    submit(&mut context, label, instruction).await
}

// ---------------------------------------------------------------------------
// The two-sided conviction, on the shipped ELF
// ---------------------------------------------------------------------------

/// A well-formed split on a FOUNDED REFUNDING Market refuses `Economic`.
///
/// Every account is at its own derived address, the request conserves, the
/// aggregate is what `founding_v5` writes, and the actor signs. The route still
/// refuses -- at `market_hoard`, the economic-slice kernel's reader, given the
/// LBV2 bytes the LBV2 reader on the line above had just accepted.
#[tokio::test]
async fn a_conserving_split_on_a_founded_refunding_market_refuses_economic() {
    let outcome = run(
        AggregateBodyV1::LiabilityBasisV2,
        "claims conservation: a conserving split on an LBV2 aggregate",
    )
    .await;
    assert!(
        !outcome.accepted,
        "the conservation route cannot accept a split: its aggregate has two \
         readers from two account families",
    );
    assert_eq!(
        outcome.refusal,
        Some(ClaimsSbfError::Economic as u32),
        "the refusal is the economic-slice kernel's, reached over LBV2 bytes; \
         logs: {:?}",
        outcome.logs,
    );
    println!(
        "conservation split (LBV2 aggregate): refused {:#06x}, {} CU consumed",
        ClaimsSbfError::Economic as u32,
        outcome.units,
    );
}

/// The SAME frame with the other family's aggregate refuses one line earlier.
///
/// One account's bytes are the only difference, and the refusal moves from the
/// economic-slice reader to the LBV2 one. Two refusals over one frame is what
/// makes this a disagreement between two authorities rather than a wrong
/// fixture: correcting the aggregate for either reader breaks the other, and
/// there is no third aggregate.
#[tokio::test]
async fn the_same_frame_with_an_economic_slice_aggregate_refuses_identity() {
    let outcome = run(
        AggregateBodyV1::EconomicSlice,
        "claims conservation: the same split on an economic-slice aggregate",
    )
    .await;
    assert!(!outcome.accepted);
    assert_eq!(
        outcome.refusal,
        Some(ClaimsSbfError::Identity as u32),
        "with economic-slice bytes the LBV2 reader is the one that refuses, \
         one line before the kernel's; logs: {:?}",
        outcome.logs,
    );
    assert_ne!(
        ClaimsSbfError::Identity as u32,
        ClaimsSbfError::Economic as u32,
        "the two refusals must be distinguishable or this pair proves nothing",
    );
    println!(
        "conservation split (economic-slice aggregate): refused {:#06x}, {} CU consumed",
        ClaimsSbfError::Identity as u32,
        outcome.units,
    );
}

// ---------------------------------------------------------------------------
// Why no third aggregate exists
// ---------------------------------------------------------------------------

/// No aggregate BYTES satisfy both of the route's two readers.
///
/// The ELF campaigns above show the route refusing twice; this is the reason,
/// stated where it can be read without a bank. It is a permanent property of
/// the two families' encodings, not a fact about this fixture.
#[test]
fn no_aggregate_bytes_satisfy_both_of_the_routes_readers() {
    use dclutch_product::economic_slice::market_hoard;
    let (_, world) = world(AggregateBodyV1::LiabilityBasisV2);
    let mut liability = world.shared.claims_market_bytes.clone();
    put_narrow_market_supplies_v2(
        &mut liability,
        &vec![FOUNDED_SETS; usize::try_from(CLAIM_COUNT).expect("width")],
    )
    .expect("founded supplies");
    let slice = economic_slice_aggregate_bytes(&world.shared, world.release_set);

    LiabilityBasisMarketViewV2::decode(&liability).expect("the LBV2 reader accepts LBV2 bytes");
    assert!(
        market_hoard(&liability).is_err(),
        "the economic-slice reader must refuse LBV2 bytes",
    );
    assert_eq!(
        market_hoard(&slice),
        Ok(FOUNDED_SETS),
        "the economic-slice reader accepts economic-slice bytes",
    );
    assert!(
        LiabilityBasisMarketViewV2::decode(&slice).is_err(),
        "the LBV2 reader must refuse economic-slice bytes",
    );
    assert_ne!(
        liability.len(),
        slice.len(),
        "at one width the two families do not even agree on the account's size, \
         so no allocation can hold both",
    );
}

/// And no Position bytes do either.
///
/// The aggregate refuses first, so the ELF campaigns above never reach the
/// Position half. It has the same defect: `ProtocolPositionSeedsV2` addresses
/// an LBV2 Position, `PositionViewV2` decodes one, and `position_native` /
/// `position_revision` / `execute_basket` read an economic-slice one.
#[test]
fn no_position_bytes_satisfy_both_of_the_routes_readers() {
    use dclutch_product::economic_slice::{
        POSITION_HEADER_BYTES, SCALAR_BYTES, initialize_position, position_native,
    };
    let (_, world) = world(AggregateBodyV1::LiabilityBasisV2);
    let count = usize::try_from(CLAIM_COUNT).expect("width");
    let failure = CLAIM_COUNT - 1;
    let mut balances = vec![0_u64; count];
    *balances.get_mut(count - 1).expect("failure coordinate") = FOUNDED_SETS;
    let escrow_owner = escrow_owner_v1(world.shared.core_market, failure);
    let liability = compile_narrow_position_v2(
        CLAIMS_PROGRAM_ID,
        world.shared.claims_market,
        escrow_owner,
        world.shared.semantic_basis_id,
        &balances,
        POSITION_REVISION,
    )
    .expect("escrow Position")
    .bytes;

    let mut slice = vec![0_u8; POSITION_HEADER_BYTES + count * 2 * SCALAR_BYTES];
    initialize_position(
        &mut slice,
        world.shared.core_market.to_bytes(),
        escrow_owner.to_bytes(),
        CLAIM_COUNT,
    )
    .expect("canonical economic-slice Position");

    LiabilityBasisPositionViewV2::decode(&liability)
        .expect("the LBV2 reader accepts LBV2 Position bytes");
    assert!(
        position_native(&liability, CLAIM_COUNT, failure).is_err(),
        "the economic-slice reader must refuse an LBV2 Position",
    );
    assert_eq!(
        position_native(&slice, CLAIM_COUNT, failure),
        Ok(0),
        "the economic-slice reader accepts economic-slice Position bytes",
    );
    assert!(
        LiabilityBasisPositionViewV2::decode(&slice).is_err(),
        "the LBV2 reader must refuse an economic-slice Position",
    );
    // THE WIDTHS COINCIDE HERE AND THAT IS NOT REASSURANCE. At `claim_count`
    // four, `128 + 4*8` and `96 + 4*16` are both 160, so a length check would
    // pass over two accounts that share not one field. The first draft of this
    // test asserted the lengths differ, as the aggregates' do, and went red on
    // its own claim -- which is why the discriminating property is stated as
    // the MAGIC, the thing that actually separates the two families at every
    // width.
    assert_eq!(
        liability.len(),
        slice.len(),
        "at width four, only by accident"
    );
    assert_ne!(
        liability.get(..8),
        slice.get(..8),
        "the two Position families are separated by their magic, at every width",
    );
}

/// The join itself: a founded refunding Market whose two Positions sum to one
/// complete set at every coordinate, beside a real Custody HoardPrincipal vault.
///
/// This is the fixture the refunding walk was owed and did not have. It asserts
/// the founding-time layout `founding_v5`'s `refunding_founding_vectors_v1`
/// produces, read back off the accounts this world plants rather than off the
/// function -- so the fixture and the route agree about what a founded
/// refunding Market looks like without either restating the other.
#[test]
fn the_joined_fixture_is_a_founded_refunding_market_with_a_custody_hoard() {
    let (_, world) = world(AggregateBodyV1::LiabilityBasisV2);
    let count = usize::try_from(CLAIM_COUNT).expect("width");
    let failure = CLAIM_COUNT - 1;
    let founder = Pubkey::new_from_array([0x31; 32]);
    let mut founder_balances = vec![FOUNDED_SETS; count];
    *founder_balances.get_mut(count - 1).expect("failure") = 0;
    let founder_position = compile_narrow_position_v2(
        CLAIMS_PROGRAM_ID,
        world.shared.claims_market,
        founder,
        world.shared.semantic_basis_id,
        &founder_balances,
        POSITION_REVISION,
    )
    .expect("founder Position");
    let escrow_owner = escrow_owner_v1(world.shared.core_market, failure);
    let mut escrow_balances = vec![0_u64; count];
    *escrow_balances.get_mut(count - 1).expect("failure") = FOUNDED_SETS;
    let escrow = compile_narrow_position_v2(
        CLAIMS_PROGRAM_ID,
        world.shared.claims_market,
        escrow_owner,
        world.shared.semantic_basis_id,
        &escrow_balances,
        POSITION_REVISION,
    )
    .expect("escrow Position");

    let founder_view =
        LiabilityBasisPositionViewV2::decode(&founder_position.bytes).expect("founder view");
    let escrow_view = LiabilityBasisPositionViewV2::decode(&escrow.bytes).expect("escrow view");
    for coordinate in 0..CLAIM_COUNT {
        let held = founder_view
            .balance(&founder_position.bytes, coordinate)
            .expect("founder balance")
            + escrow_view
                .balance(&escrow.bytes, coordinate)
                .expect("escrow balance");
        assert_eq!(
            held, FOUNDED_SETS,
            "the two Positions sum to one complete set at coordinate {coordinate}",
        );
    }
    assert_eq!(
        founder_view
            .balance(&founder_position.bytes, failure)
            .expect("founder failure balance"),
        0,
        "the founder holds NO failure claim on a refunding Market",
    );
    assert_eq!(
        escrow.account, world.escrow_position,
        "the escrow the world plants is the Market's own derived one",
    );
    assert_eq!(
        world.shared.payout_scale,
        u64::from(CLAIM_COUNT - 1),
        "and the record, not a caller, is what says the Market refunds",
    );
    assert_ne!(
        world.hoard, world.custody_authority,
        "the HoardPrincipal vault and the Custody transfer authority are two \
         distinct derived accounts",
    );
}
