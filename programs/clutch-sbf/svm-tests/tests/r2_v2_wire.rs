//! The v2 pull source, founded, ingested, sealed and resolved on a real bank.
//!
//! `r2_pull_endow.rs` showed the default ELF taking custody against a v2
//! SourceSpec that was *installed at genesis*, because no instruction could
//! create one. This file closes that circle. Every state transition below is a
//! real transaction against the real SBF ELF, through the four new layout tags:
//!
//! | tag | intent | what it does here |
//! | ---: | --- | --- |
//! | 70 | `InitSourceSpecV2` | founds the 404-byte spec account and its feed head |
//! | 71 | `InitSourceArchiveV2` | founds the 2,560-byte v2 archive page |
//! | 72 | `AppendSourceArchiveV2` | admits one authenticated pull record per boundary |
//! | 73 | `SealSourceArchiveV2` | seals the complete window and advances the feed |
//!
//! and then the market resolves from the page the program itself built, and a
//! holder redeems against that resolution.
//!
//! ## The laboratory receiver, and exactly what it stands for
//!
//! An append is only admissible when the *immediately preceding instruction in
//! the same transaction* invoked the pinned receiver program naming this exact
//! ephemeral update account. That adjacency is read out of the Instructions
//! sysvar, which the runtime synthesizes — it cannot be fabricated. So the
//! transaction really does have to contain a real, successful instruction to a
//! real, loadable program at the pinned receiver address.
//!
//! `clutch_lab_receiver.so` is that program: a narrow account writer,
//! installed at `source_identity::fixture::RECEIVER_PROGRAM` under a fabricated
//! Upgradeable Loader program/ProgramData pair. It is **not** a model of Pyth's
//! proof verification: it only copies a canonical 134-byte body into the
//! receiver-owned update account. That deliberately proves the transaction
//! seam the former no-op fixture skipped: the adjacent instruction writes the
//! evidence Dragon consumes, and a later refusal rolls both writes back.
//!
//! ## Two deliberate deviations from the fixture identity, both named
//!
//! * **Deployment slot 1, not `fixture::PROGRAMDATA_DEPLOYMENT_SLOT`
//!   (8,421,504).** A program is invisible to the runtime until one slot after
//!   its recorded deployment, and a bank whose genesis is slot 0 would need an
//!   8.4-million-slot warp to see one deployed there. The slot is not part of
//!   the registry match; what the join checks is that the *spec* and the
//!   *ProgramData account* record the same one, and here they both record 1.
//! * **Window buckets derived from the live Clock.** `CROSSING_V1` admits the
//!   record for bucket `b` only once Clock has passed `(b+1)·60 + grace`, and
//!   the update's publish time must sit inside the spec's freshness envelope
//!   against that same Clock. The fixture window at buckets 100..103 is
//!   1970-01-01; a bank's clock is not. So the window is computed from the
//!   Clock the bank actually reports and the whole plane is installed against
//!   it. Nothing about the *rule* is relaxed — the grace, the staleness bounds
//!   and the future-skew allowance are the spec's own.
//!
//! ## What this is not
//!
//! Laboratory evidence that the runtime path is correct. The spec's identity is
//! `source_identity::fixture`, whose receiver program is an address no party can
//! deploy to; `source_identity::mainnet` is still entirely empty. No production
//! byte is pinned here.

use {
    clutch_kernel::{PayoutSet, PayoutVector},
    clutch_sbf::{
        error::ClutchError,
        instructions::observe_resolve,
        loader_state::UPGRADEABLE_LOADER_ID,
        pyth_receiver::PRICE_UPDATE_V2_ACCOUNT_LEN,
        seeds,
        source_archive_v2::{
            ARCHIVE_COMMITMENT_OFFSET, SOURCE_ARCHIVE_ACCOUNT_V2_BYTES,
            SOURCE_SPEC_ACCOUNT_V2_BYTES,
        },
        source_identity::fixture,
        source_v2::{
            crossing::SELECTION_CROSSING_V1,
            fixtures::{
                config_body, price_update_body, programdata_body, receiver_program_body,
                PriceUpdateFixture,
            },
            spec::{
                SourceSpecFieldsV2, SourceSpecV2, GRID_ORIGIN_UNIX_SECONDS_V1,
                ORIENTATION_QUOTE_PER_BASE,
            },
        },
    },
    clutch_solana_layout::{
        account_len, canonical_outcome_id,
        occupation_resolution::{
            OccupationResolutionAccount, OCCUPATION_RESOLUTION_LEN,
            RESOLUTION_MODE_DERIVED_QUANTIZED_OCCUPATION, STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06,
        },
        FeedAccount, Hash32, HoardAccount, Intent, MarketAccount, PayoutVectorBytes,
        PositionAccount, ResolutionAccount, SupplyLedgerAccount, TermsAccount, MAX_KNOTS,
        MAX_OUTCOMES, MAX_PAYOUTS, PAYOUT_INDEX_UNRESOLVED, PAYOUT_MAP_UNUSED,
    },
    clutch_solana_reference::{KernelAccount, STAT_TERMINAL_01},
    clutch_svm_fixture::{
        build_plane, compute_unit_limit_data, fixture_terms, immutable_owner_account_bytes,
        layout_request, outcome_mint_bytes, token_account_bytes, GenesisAccount, Mode, Pda, Plane,
        COMPUTE_BUDGET, MARKET_NONCE, PROGRAM_ID, RENT_SYSVAR, SYSTEM_PROGRAM, TOKEN_2022,
    },
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{error::InstructionError, AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_program_test::{tokio, ProgramTest, ProgramTestContext},
    solana_rent::Rent,
    solana_signer::Signer,
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
};

#[cfg(feature = "non-production-real-pyth-lab")]
use clutch_sbf::source_identity::real_pyth_lab;
#[cfg(feature = "non-production-real-pyth-lab")]
use solana_clock::Clock;
#[cfg(feature = "non-production-real-pyth-lab")]
use solana_system_interface::instruction as system_instruction;

/* ------------------------------------------------------------------------ */
/* Shape of the market this campaign resolves                                */
/* ------------------------------------------------------------------------ */

/// Outcomes, payouts, and the quantized denominator of the occupation vector.
const OUTCOMES: u8 = 4;
const DENOMINATOR: u64 = 64;
/// Complete sets the founding position holds.
const SETS: u64 = 64;
/// Degree of the native basis this market resolves under.
const BASIS_DEGREE: u8 = 1;
/// Buckets in the observation window; one authenticated append each.
const SPAN: u64 = 3;
/// The exact normalized price atoms every admitted record carries.
///
/// With `normalized_decimals = 8` and a `-8` exponent the normalization is the
/// identity, and a zero confidence makes the conservative interval a point —
/// which the occupation fold requires and refuses to invent.
const PRICE_ATOMS: i64 = 4;
/// Deployment slot both the spec and the fabricated ProgramData record.
const DEPLOYMENT_SLOT: u64 = 1;
/// Slot the campaign warps to before it does anything else.
///
/// Exactly one slot past [`DEPLOYMENT_SLOT`], and that is the whole
/// requirement, twice over. A program is invisible until one slot past its
/// recorded deployment, so slot 2 is the first at which the laboratory receiver
/// is effective. And the warp roots slot 1, which is what puts the cache entry
/// on this fork: the program cache treats an entry as reachable when its
/// deployment slot is at or below the root, and a deployment slot naming a
/// pruned, never-rooted slot makes every invocation re-load the program.
const WARP_SLOT: u64 = DEPLOYMENT_SLOT + 1;

#[cfg(feature = "non-production-real-pyth-lab")]
const REAL_PYTH_WARP_SLOT: u64 = real_pyth_lab::RECEIVER_DEPLOYMENT_SLOT + 1;
#[cfg(feature = "non-production-real-pyth-lab")]
const REAL_PYTH_ROUTER_WARP_SLOT: u64 = real_pyth_lab::ROUTER_DEPLOYMENT_SLOT + 1;
#[cfg(feature = "non-production-real-pyth-lab")]
const REAL_PYTH_PUBLISH_TIME: i64 = 1_787_431_680;

const ACTOR_TOKEN: Address = Address::new_from_array([0x8e; 32]);
/// A second owner, with no position, who takes custody after the founding.
const ENDOW_OWNER_TOKEN: Address = Address::new_from_array([0x8f; 32]);
/// Collateral atoms that owner deposits.
const DEPOSIT: u64 = 500;
const COLLATERAL_MINT: Address = Address::new_from_array([0x6c; 32]);

const CU_LIMIT: u32 = 1_400_000;

fn endow_owner_keypair() -> Keypair {
    Keypair::new_from_array([
        0x71, 0x08, 0xd4, 0x39, 0xb6, 0x2f, 0x83, 0x15, 0xca, 0x64, 0x20, 0x9e, 0x47, 0xf1, 0x5b,
        0x32, 0xad, 0x06, 0x78, 0xc3, 0x11, 0xe5, 0x59, 0x24, 0x8a, 0x4d, 0x90, 0x37, 0xfb, 0x6e,
        0x02, 0xac,
    ])
}

fn actor_keypair() -> Keypair {
    Keypair::new_from_array([
        0x77, 0x19, 0x42, 0xa8, 0x51, 0x0e, 0xf3, 0x22, 0x63, 0x99, 0x14, 0xc0, 0x2d, 0x6b, 0x84,
        0x31, 0x7a, 0x55, 0xd8, 0x0b, 0xe2, 0x40, 0x6f, 0x91, 0x13, 0xcc, 0x75, 0x28, 0x9d, 0x04,
        0xb6, 0x5e,
    ])
}

fn update_keypair() -> Keypair {
    Keypair::new_from_array([
        0x31, 0xa7, 0x04, 0xd8, 0x56, 0x12, 0x9e, 0x43, 0x85, 0x2c, 0x61, 0xba, 0xf0, 0x77, 0x19,
        0x3d, 0xca, 0x28, 0x90, 0x5e, 0x44, 0x0b, 0xd1, 0x68, 0x27, 0xec, 0x95, 0x32, 0x7f, 0x11,
        0xa6, 0x59,
    ])
}

fn decoy_update_keypair() -> Keypair {
    Keypair::new_from_array([
        0x82, 0x16, 0xcb, 0x45, 0x03, 0x9f, 0xe7, 0x2a, 0xb4, 0x65, 0x10, 0x73, 0xd8, 0x29, 0x5c,
        0xf1, 0x47, 0x90, 0x2d, 0xa3, 0x6e, 0x0c, 0x58, 0xbb, 0x24, 0x79, 0xd0, 0x13, 0x9a, 0x4f,
        0xe5, 0x36,
    ])
}

#[cfg(feature = "non-production-real-pyth-lab")]
fn encoded_vaa_keypair() -> Keypair {
    Keypair::new_from_array([
        0x93, 0x11, 0xa4, 0x70, 0x2b, 0x59, 0xc8, 0x05, 0x61, 0xdd, 0x7e, 0x14, 0xb2, 0x37, 0x98,
        0x4a, 0xe5, 0x20, 0x6c, 0xf1, 0x89, 0x42, 0x0d, 0xba, 0x73, 0x18, 0xce, 0x54, 0x07, 0x9f,
        0x2d, 0x66,
    ])
}

#[cfg(feature = "non-production-real-pyth-lab")]
fn is_real_pyth_spec(spec: SourceSpecV2) -> bool {
    spec.fields().receiver_program == real_pyth_lab::RECEIVER_PROGRAM
}

#[cfg(not(feature = "non-production-real-pyth-lab"))]
fn is_real_pyth_spec(_spec: SourceSpecV2) -> bool {
    false
}

fn pda(seeds: &[&[u8]]) -> Pda {
    let (address, bump) = Address::find_program_address(seeds, &PROGRAM_ID);
    Pda { address, bump }
}

fn encode<F, E>(len: usize, encoder: F) -> Vec<u8>
where
    F: FnOnce(&mut [u8]) -> Result<usize, E>,
    E: core::fmt::Debug,
{
    let mut out = vec![0_u8; len];
    assert_eq!(encoder(&mut out).expect("fixture encodes"), len);
    out
}

fn account_mut(plane: &mut Plane, address: Address) -> &mut GenesisAccount {
    plane
        .accounts
        .iter_mut()
        .find(|account| account.address == address)
        .expect("fixture account exists")
}

fn one_hot_payouts() -> ([PayoutVectorBytes; MAX_PAYOUTS], PayoutSet) {
    let mut bytes = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
    let mut kernel = [PayoutVector::ZERO; MAX_PAYOUTS];
    for outcome in 0..usize::from(OUTCOMES) {
        let mut weights = [0_u64; MAX_OUTCOMES];
        weights[outcome] = DENOMINATOR;
        bytes[outcome] = PayoutVectorBytes {
            denominator: DENOMINATOR,
            weights,
        };
        kernel[outcome] = PayoutVector::new(DENOMINATOR, weights);
    }
    (bytes, PayoutSet::new(OUTCOMES, OUTCOMES, kernel))
}

/* ------------------------------------------------------------------------ */
/* The v2 spec this campaign founds                                          */
/* ------------------------------------------------------------------------ */

fn pull_spec_fields() -> SourceSpecFieldsV2 {
    SourceSpecFieldsV2 {
        source_adapter_id: fixture::SOURCE_ADAPTER_ID,
        source_adapter_version: fixture::SOURCE_ADAPTER_VERSION,
        parser_id: fixture::PARSER_ID,
        parser_version: fixture::PARSER_VERSION,
        receiver_program: fixture::RECEIVER_PROGRAM,
        receiver_programdata: fixture::RECEIVER_PROGRAMDATA,
        receiver_config: fixture::RECEIVER_CONFIG,
        config_digest: config_digest(),
        provider_feed_id: fixture::PROVIDER_FEED_ID,
        programdata_deployment_slot: DEPLOYMENT_SLOT,
        base_asset_id: fixture::BASE_ASSET_ID,
        quote_asset_id: fixture::QUOTE_ASSET_ID,
        orientation: ORIENTATION_QUOTE_PER_BASE,
        normalized_decimals: 8,
        grid_family_id: 7,
        grid_version: 1,
        grid_origin_unix_seconds: GRID_ORIGIN_UNIX_SECONDS_V1,
        bucket_seconds: 60,
        boundary_grace_seconds: 5,
        max_staleness_slots: 500,
        max_staleness_seconds: 600,
        max_future_seconds: 15,
        max_confidence_atoms: 1_000_000_000_000,
        max_confidence_bps: 500,
        confidence_multiplier: 3,
        selection_rule: SELECTION_CROSSING_V1,
    }
}

fn registered_spec() -> SourceSpecV2 {
    SourceSpecV2::new(pull_spec_fields()).expect("the fixture pull spec is valid")
}

/// A structurally valid spec naming a parser release this ELF does not carry.
fn unregistered_spec() -> SourceSpecV2 {
    let mut fields = pull_spec_fields();
    fields.parser_version += 1;
    SourceSpecV2::new(fields).expect("still a valid v2 spec")
}

/* ------------------------------------------------------------------------ */
/* The fabricated receiver deployment                                        */
/* ------------------------------------------------------------------------ */

fn receiver_config_bytes() -> Vec<u8> {
    config_body(
        [0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48],
        [0x51; 32],
        None,
        [0x52; 32],
        &[(26, [0x53; 32])],
        1_000,
        5,
    )
}

fn config_digest() -> [u8; 32] {
    clutch_sbf::pyth_receiver::config_byte_digest(&receiver_config_bytes())
}

#[cfg(feature = "non-production-real-pyth-lab")]
fn real_pyth_fixture(name: &str) -> Vec<u8> {
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/real-pyth-local")
            .join(name),
    )
    .unwrap_or_else(|error| panic!("real-Pyth fixture {name} must exist: {error}"));
    let expected = match name {
        "receiver.so" => "c5079559864fc34dbd5fe87b4aa9fba3a1ed22690363ec490449e8660e73af64",
        "router.so" => "f9061f03a81b89db29f4603677e3b3d89b3bbf08d67827b2832f18a4e2b61acb",
        "router-initialize.data" => {
            "3667940a4428a8f2411a0ff11157ecc4ba1076c3c61273a108da6405c51e0b0b"
        }
        "receiver-initialize.data" => {
            "d9c80906af92f99a0c8441f4463186056b1c12cb990999acfa198a46ec62729f"
        }
        "receiver-config.account" => {
            "05038cf707afceac3df1aae735b096344ad639506b00f1db0ac1c084d6b645aa"
        }
        "signed.vaa" => "ed8b973f36a932b9ec88659953859c8096f14e5aebd085bbe32b22c41a142c0d",
        "receiver-post-update.data" => {
            "3bf9188bd6183155ea30738c3ab9da706ea7013bf5a7887a531e90b9bea85e1d"
        }
        "price-update.account" => {
            "e5435e5b2e54d6083a9d1230e33f0635f6c74eb9db62899cfbb559f99c798a2b"
        }
        other => panic!("fixture {other} has no executable SHA-256 pin"),
    };
    let actual: String = clutch_sbf::pyth_receiver::config_byte_digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(actual, expected, "real-Pyth fixture digest: {name}");
    bytes
}

#[cfg(feature = "non-production-real-pyth-lab")]
fn real_pyth_spec(feed_id: [u8; 32]) -> SourceSpecV2 {
    let mut fields = pull_spec_fields();
    fields.receiver_program = real_pyth_lab::RECEIVER_PROGRAM;
    fields.receiver_programdata = real_pyth_lab::RECEIVER_PROGRAMDATA;
    fields.receiver_config = real_pyth_lab::RECEIVER_CONFIG;
    fields.config_digest = clutch_sbf::pyth_receiver::config_byte_digest(&real_pyth_fixture(
        "receiver-config.account",
    ));
    fields.provider_feed_id = feed_id;
    fields.programdata_deployment_slot = real_pyth_lab::RECEIVER_DEPLOYMENT_SLOT;
    fields.base_asset_id = real_pyth_lab::BASE_ASSET_ID;
    fields.quote_asset_id = real_pyth_lab::QUOTE_ASSET_ID;
    SourceSpecV2::new(fields).expect("local-real Pyth spec is structurally valid")
}

#[cfg(feature = "non-production-real-pyth-lab")]
fn assert_real_pyth_deployment_bytes(
    receiver_program: &[u8],
    receiver_programdata: &[u8],
    router_program: &[u8],
    router_programdata: &[u8],
) {
    use clutch_sbf::pyth_receiver::config_byte_digest as sha256;
    assert_eq!(
        sha256(receiver_program),
        [
            0xef, 0x37, 0xdd, 0x1c, 0xee, 0x22, 0xd7, 0x31, 0x90, 0x2a, 0x8c, 0x04, 0xed, 0x2e,
            0x13, 0x13, 0x6a, 0x2b, 0x8a, 0xa7, 0x06, 0x8d, 0x9d, 0xb3, 0xaf, 0xf2, 0xed, 0x1e,
            0xc7, 0xb6, 0x34, 0xe5,
        ]
    );
    assert_eq!(
        sha256(receiver_programdata),
        [
            0x71, 0x22, 0xab, 0xc6, 0xb5, 0xe7, 0x8d, 0x30, 0xbf, 0x88, 0xc8, 0x69, 0xcb, 0x5d,
            0x87, 0x83, 0xad, 0xaf, 0x89, 0x73, 0x69, 0xd0, 0x4e, 0xca, 0x82, 0x7d, 0x3a, 0xf8,
            0xff, 0xe1, 0x8e, 0x5d,
        ]
    );
    assert_eq!(
        sha256(router_program),
        [
            0x1e, 0xe5, 0x90, 0xae, 0x23, 0xd5, 0xec, 0xbf, 0x77, 0x5a, 0xba, 0x91, 0x0f, 0x06,
            0xa9, 0x93, 0xde, 0xe8, 0xf7, 0x7b, 0xfd, 0x70, 0x28, 0x79, 0x0d, 0xbd, 0x34, 0x96,
            0x51, 0xc8, 0x03, 0x4b,
        ]
    );
    assert_eq!(
        sha256(router_programdata),
        [
            0xf2, 0x6f, 0x4b, 0x53, 0xb0, 0xf9, 0x80, 0x45, 0x58, 0x86, 0x11, 0x6f, 0x50, 0x0f,
            0xa7, 0x4b, 0xa4, 0x75, 0xe5, 0x1b, 0x1a, 0xcb, 0x7f, 0x48, 0x6b, 0x18, 0xaf, 0xa9,
            0xd7, 0x3d, 0x94, 0x8f,
        ]
    );
}

fn lab_receiver_elf() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/clutch_lab_receiver.so");
    std::fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "the laboratory receiver ELF must be built into {}: {error}\n\
             build it with `cargo-build-sbf --manifest-path lab-receiver/Cargo.toml \\\n\
             --sbf-out-dir tests/fixtures` (run_svm_tests.sh does this)",
            path.display()
        )
    })
}

fn genesis_account(data: Vec<u8>, owner: Address, executable: bool) -> Account {
    Account {
        lamports: Rent::default().minimum_balance(data.len()).max(1),
        data,
        owner,
        executable,
        rent_epoch: 0,
    }
}

/* ------------------------------------------------------------------------ */
/* The market plane, built around a live-Clock window                        */
/* ------------------------------------------------------------------------ */

/// Everything the campaign needs to address one v2-bound market.
struct PullPlane {
    plane: Plane,
    spec: SourceSpecV2,
    start_bucket: u64,
    end_bucket_exclusive: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PullMarketKind {
    Occupation,
    Categorical { knots: [u128; 3] },
}

/// Build a market bound to one v2 pull spec and one window, with the source
/// spec, feed head and archive **absent**: those are exactly what the new
/// intents create.
fn pull_plane(
    actor: Address,
    spec: SourceSpecV2,
    start_bucket: u64,
    end_bucket_exclusive: u64,
    kind: PullMarketKind,
) -> PullPlane {
    let span = end_bucket_exclusive
        .checked_sub(start_bucket)
        .filter(|span| *span > 0)
        .expect("the pull window must contain at least one bucket");
    let feed_id = Hash32::from_bytes(spec.feed_id());
    let mut plane = build_plane(actor, COLLATERAL_MINT, MARKET_NONCE, Mode::Funded);

    let old_terms_address = plane.terms.address;
    let market_address = plane.market.address;
    let position_address = plane.position.address;
    let kernel_address = plane.kernel.address;
    let supply_address = plane.supply.address;
    let hoard_address = plane.hoard.address;
    let resolution_address = plane.resolution.address;
    let (payout_bytes, payout_set) = one_hot_payouts();

    /* Terms is self-certifying: the digest is over the body and the address is
     * terms_pda(realm, digest), with the stored bump outside the body. */
    let mut terms = fixture_terms(plane.realm_id, plane.profile_id, feed_id);
    terms.source_adapter_id = Hash32::from_bytes(fixture::SOURCE_ADAPTER_ID);
    terms.source_version = fixture::SOURCE_ADAPTER_VERSION;
    terms.outcome_count = OUTCOMES;
    terms.payout_count = OUTCOMES;
    terms.payouts = payout_bytes;
    terms.knots = [0; MAX_KNOTS];
    terms.payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
    match kind {
        PullMarketKind::Occupation => {
            terms.statistic_id = STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06;
            terms.basis_degree = BASIS_DEGREE;
            terms.knot_count = OUTCOMES + 1 - BASIS_DEGREE;
            terms.uniform_log2_spacing = 3;
            for (index, knot) in terms
                .knots
                .iter_mut()
                .take(usize::from(terms.knot_count))
                .enumerate()
            {
                *knot = (index as u128) * 8;
            }
        }
        PullMarketKind::Categorical { knots } => {
            terms.statistic_id = STAT_TERMINAL_01;
            terms.basis_degree = 0;
            terms.knot_count = OUTCOMES - 1;
            terms.uniform_log2_spacing = clutch_solana_layout::UNIFORM_SPACING_NONE;
            terms.knots[..knots.len()].copy_from_slice(&knots);
            for payout in 0..OUTCOMES {
                terms.payout_map[usize::from(payout)] = payout;
            }
        }
    }
    terms.expected_start_bucket = start_bucket;
    terms.expected_end_bucket_exclusive = end_bucket_exclusive;
    /* `read_frozen_terms` requires the maturity bucket to be exactly one past
     * the window end, so the horizon is the span plus one. */
    terms.maturity_horizon_buckets = span + 1;
    terms.terms = Hash32::ZERO;
    terms.terms = terms
        .recomputed_terms_digest()
        .expect("the pull occupation terms body encodes");
    let terms_id = terms.terms;
    let realm_seed = plane.realm_id.bytes();
    let terms_pda = pda(&[seeds::SEED_TERMS, &realm_seed, &terms_id.bytes()]);
    terms.stored_bump = terms_pda.bump;
    assert_eq!(
        terms
            .recomputed_terms_digest()
            .expect("the terms body encodes"),
        terms_id,
        "the stored bump must be outside the digest body"
    );
    let terms_account = account_mut(&mut plane, old_terms_address);
    terms_account.address = terms_pda.address;
    terms_account.data = encode(account_len::TERMS, |out| terms.encode(out));
    plane.terms = terms_pda;
    plane.terms_id = terms_id;

    let market_id = plane.market_id;
    let market_seed = market_id.bytes();
    let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
    plane.outcome_mints.clear();
    for outcome in 0..OUTCOMES {
        outcomes[usize::from(outcome)] = canonical_outcome_id(market_id, outcome);
        plane
            .outcome_mints
            .push(pda(&[seeds::SEED_OUTCOME_MINT, &market_seed, &[outcome]]));
    }
    let mut market =
        MarketAccount::decode(&account_mut(&mut plane, market_address).data).expect("market");
    market.terms = terms_id;
    market.feed = feed_id;
    market.outcome_count = OUTCOMES;
    market.outcomes = outcomes;
    account_mut(&mut plane, market_address).data =
        encode(account_len::MARKET, |out| market.encode(out));

    let mut internal = [0_u64; MAX_OUTCOMES];
    internal[..usize::from(OUTCOMES)].fill(SETS);
    let mut position =
        PositionAccount::decode(&account_mut(&mut plane, position_address).data).expect("position");
    position.internal = internal;
    account_mut(&mut plane, position_address).data =
        encode(account_len::POSITION, |out| position.encode(out));

    let mut total_supply = [0_u64; MAX_OUTCOMES];
    total_supply[..usize::from(OUTCOMES)].fill(SETS);
    let kernel = KernelAccount {
        market: market_id,
        phase: 0,
        basis_mode: match kind {
            PullMarketKind::Occupation => clutch_kernel::BasisMode::DerivedBasis,
            PullMarketKind::Categorical { .. } => clutch_kernel::BasisMode::FinitePreset,
        },
        resolved_payout: 0,
        payouts: payout_set,
        total_supply,
    };
    account_mut(&mut plane, kernel_address).data =
        encode(clutch_solana_reference::KERNEL_ACCOUNT_LEN, |out| {
            kernel.encode(out)
        });

    let mut supply =
        SupplyLedgerAccount::decode(&account_mut(&mut plane, supply_address).data).expect("supply");
    supply.outcome_count = OUTCOMES;
    supply.internal_supply = internal;
    supply.external_supply = [0; MAX_OUTCOMES];
    account_mut(&mut plane, supply_address).data =
        encode(account_len::SUPPLY_LEDGER, |out| supply.encode(out));

    let mut hoard =
        HoardAccount::decode(&account_mut(&mut plane, hoard_address).data).expect("hoard decodes");
    hoard.collateral_atoms = SETS;
    account_mut(&mut plane, hoard_address).data =
        encode(account_len::HOARD, |out| hoard.encode(out));
    plane.hoard_atoms = SETS;

    match kind {
        PullMarketKind::Occupation => {
            let unresolved = OccupationResolutionAccount::unresolved(
                market_id,
                terms_id,
                feed_id,
                plane.resolution.bump,
            );
            account_mut(&mut plane, resolution_address).data =
                encode(OCCUPATION_RESOLUTION_LEN, |out| unresolved.encode(out));
        }
        PullMarketKind::Categorical { .. } => {
            let unresolved = ResolutionAccount {
                market: market_id,
                terms: terms_id,
                feed: feed_id,
                window: Hash32::ZERO,
                feed_cursor: 0,
                sealed_end_bucket_exclusive: 0,
                repair_generation: 0,
                resolved_slot: 0,
                payout_index: PAYOUT_INDEX_UNRESOLVED,
                stored_bump: plane.resolution.bump,
                flags: 0,
            };
            account_mut(&mut plane, resolution_address).data =
                encode(account_len::RESOLUTION, |out| unresolved.encode(out));
        }
    }

    /* The three accounts this campaign's own instructions create are removed
     * from the plane rather than rewritten: an installed image would make the
     * creating instruction unobservable. */
    let spec_pda = pda(&[seeds::SEED_SOURCE_SPEC, &feed_id.bytes()]);
    let feed_pda = pda(&[seeds::SEED_FEED, &feed_id.bytes()]);
    let window_id = window_identity(&terms, feed_id);
    let archive_pda = pda(&[
        seeds::SEED_SOURCE_ARCHIVE,
        &feed_id.bytes(),
        &window_id.bytes(),
    ]);
    plane.accounts.retain(|account| {
        account.address != plane.source_spec.address
            && account.address != plane.feed.address
            && account.address != plane.source_archive.address
            && account.address != spec_pda.address
            && account.address != feed_pda.address
            && account.address != archive_pda.address
    });
    plane.source_spec = spec_pda;
    plane.feed = feed_pda;
    plane.source_archive = archive_pda;
    plane.feed_id = feed_id;
    plane.window_id = window_id;

    PullPlane {
        plane,
        spec,
        start_bucket,
        end_bucket_exclusive,
    }
}

fn pull_occupation_plane(actor: Address, spec: SourceSpecV2, start_bucket: u64) -> PullPlane {
    pull_plane(
        actor,
        spec,
        start_bucket,
        start_bucket + SPAN,
        PullMarketKind::Occupation,
    )
}

fn pull_categorical_plane(actor: Address, spec: SourceSpecV2, start_bucket: u64) -> PullPlane {
    pull_plane(
        actor,
        spec,
        start_bucket,
        start_bucket + SPAN,
        PullMarketKind::Categorical {
            knots: [500, 1_000, 1_500],
        },
    )
}

/// Recompute the canonical window identity the archive PDA is derived from.
fn window_identity(terms: &TermsAccount, feed: Hash32) -> Hash32 {
    let identity = clutch_sbf::source_archive::FeedIdentity::new(
        terms.source_adapter_id.bytes(),
        feed.bytes(),
        terms.source_version,
        terms.evaluator_version,
    )
    .expect("fixture feed identity");
    let grid = clutch_sbf::source_archive::Grid::new(
        terms.grid_family_id,
        terms.grid_version,
        terms.bucket_seconds,
    )
    .expect("fixture grid");
    let window = clutch_sbf::source_archive::WindowDomain::new(
        identity,
        grid,
        terms.expected_start_bucket,
        terms.expected_end_bucket_exclusive,
        terms.expected_start_bucket + terms.maturity_horizon_buckets,
        terms.repair_generation,
        clutch_sbf::source_archive::CoveragePolicy::COMPLETE_REQUIRED,
    )
    .expect("fixture window domain");
    clutch_sbf::source_archive_v2::canonical_window_id(window)
}

/* ------------------------------------------------------------------------ */
/* Harness                                                                   */
/* ------------------------------------------------------------------------ */

struct Campaign {
    context: ProgramTestContext,
    /// Distinguishes otherwise byte-identical transactions.
    ///
    /// The whole campaign runs inside one slot, so two transactions with the
    /// same instructions and the same blockhash have the same signature and the
    /// bank refuses the second as already processed. Spending one fewer
    /// compute unit each time keeps every attempt a distinct transaction
    /// without changing what any of them does — a hostile shape has to be
    /// *refused*, not deduplicated.
    nonce: u32,
    actor: Keypair,
    endow_owner: Keypair,
    update: Keypair,
    decoy_update: Keypair,
    #[cfg(feature = "non-production-real-pyth-lab")]
    encoded_vaa: Keypair,
    plane: Plane,
    spec: SourceSpecV2,
    start_bucket: u64,
    end_bucket_exclusive: u64,
}

impl Campaign {
    async fn start(spec: SourceSpecV2) -> Self {
        Self::start_with(spec, PullMarketKind::Occupation).await
    }

    async fn start_categorical(spec: SourceSpecV2) -> Self {
        Self::start_with(
            spec,
            PullMarketKind::Categorical {
                knots: [500, 1_000, 1_500],
            },
        )
        .await
    }

    async fn start_with(spec: SourceSpecV2, kind: PullMarketKind) -> Self {
        Self::start_with_window(spec, kind, None).await
    }

    #[cfg(feature = "non-production-real-pyth-lab")]
    async fn start_real_pyth_one_bucket_categorical(spec: SourceSpecV2) -> Self {
        assert!(is_real_pyth_spec(spec));
        let boundary_bucket = u64::try_from(REAL_PYTH_PUBLISH_TIME)
            .expect("the real-Pyth fixture publish time is positive")
            / 60;
        Self::start_with_window(
            spec,
            PullMarketKind::Categorical {
                /* The authenticated nonzero-confidence interval is
                 * [99,980,929, 100,019,071], wholly inside cell one. */
                knots: [99_000_000, 101_000_000, 102_000_000],
            },
            Some((boundary_bucket - 1, boundary_bucket)),
        )
        .await
    }

    async fn start_with_window(
        spec: SourceSpecV2,
        kind: PullMarketKind,
        window: Option<(u64, u64)>,
    ) -> Self {
        let actor = actor_keypair();
        let update = update_keypair();
        let decoy_update = decoy_update_keypair();
        let real_pyth = is_real_pyth_spec(spec);
        let mut test = ProgramTest::default();
        test.prefer_bpf(true);
        test.add_program("clutch_sbf", PROGRAM_ID, None);

        /* The fabricated receiver deployment goes in at genesis, not through
         * `set_account`, so the program account's last modification slot is
         * zero and the loader sees a settled deployment. */
        if !real_pyth {
            let elf = lab_receiver_elf();
            test.add_account(
                Address::new_from_array(fixture::RECEIVER_PROGRAM),
                genesis_account(
                    receiver_program_body(fixture::RECEIVER_PROGRAMDATA),
                    Address::new_from_array(UPGRADEABLE_LOADER_ID),
                    true,
                ),
            );
            test.add_account(
                Address::new_from_array(fixture::RECEIVER_PROGRAMDATA),
                genesis_account(
                    programdata_body(DEPLOYMENT_SLOT, Some([0x61; 32]), [0; 32], &elf),
                    Address::new_from_array(UPGRADEABLE_LOADER_ID),
                    false,
                ),
            );
            test.add_account(
                Address::new_from_array(fixture::RECEIVER_CONFIG),
                genesis_account(
                    receiver_config_bytes(),
                    Address::new_from_array(fixture::RECEIVER_PROGRAM),
                    false,
                ),
            );
            for key in [update.pubkey(), decoy_update.pubkey()] {
                test.add_account(
                    key,
                    genesis_account(
                        vec![0_u8; PRICE_UPDATE_V2_ACCOUNT_LEN],
                        Address::new_from_array(fixture::RECEIVER_PROGRAM),
                        false,
                    ),
                );
            }
        }
        #[cfg(feature = "non-production-real-pyth-lab")]
        if real_pyth {
            let loader = Address::new_from_array(UPGRADEABLE_LOADER_ID);
            let receiver_address = Address::new_from_array(real_pyth_lab::RECEIVER_PROGRAM);
            let router_address = Address::new_from_array(real_pyth_lab::ROUTER_PROGRAM);
            assert_eq!(
                Address::find_program_address(&[receiver_address.as_ref()], &loader).0,
                Address::new_from_array(real_pyth_lab::RECEIVER_PROGRAMDATA),
                "receiver ProgramData must be the loader's canonical PDA"
            );
            assert_eq!(
                Address::find_program_address(&[router_address.as_ref()], &loader).0,
                Address::new_from_array(real_pyth_lab::ROUTER_PROGRAMDATA),
                "router ProgramData must be the loader's canonical PDA"
            );
            let receiver_program = receiver_program_body(real_pyth_lab::RECEIVER_PROGRAMDATA);
            let receiver_programdata = programdata_body(
                real_pyth_lab::RECEIVER_DEPLOYMENT_SLOT,
                Some(real_pyth_lab::UPGRADE_AUTHORITY),
                [0; 32],
                &real_pyth_fixture("receiver.so"),
            );
            let router_program = receiver_program_body(real_pyth_lab::ROUTER_PROGRAMDATA);
            let router_programdata = programdata_body(
                real_pyth_lab::ROUTER_DEPLOYMENT_SLOT,
                Some(real_pyth_lab::UPGRADE_AUTHORITY),
                [0; 32],
                &real_pyth_fixture("router.so"),
            );
            assert_real_pyth_deployment_bytes(
                &receiver_program,
                &receiver_programdata,
                &router_program,
                &router_programdata,
            );
            for (key, data, executable) in [
                (real_pyth_lab::RECEIVER_PROGRAM, receiver_program, true),
                (
                    real_pyth_lab::RECEIVER_PROGRAMDATA,
                    receiver_programdata,
                    false,
                ),
                (real_pyth_lab::ROUTER_PROGRAM, router_program, true),
                (real_pyth_lab::ROUTER_PROGRAMDATA, router_programdata, false),
            ] {
                test.add_genesis_account(
                    Address::new_from_array(key),
                    genesis_account(data, loader, executable),
                );
            }
        }
        let mut funded = genesis_account(Vec::new(), SYSTEM_PROGRAM, false);
        funded.lamports = 10_000_000_000;
        test.add_account(actor.pubkey(), funded.clone());
        test.add_account(endow_owner_keypair().pubkey(), funded);

        let mut context = test.start_with_context().await;
        /* One small warp: a program is invisible until one slot past its
         * recorded deployment, and the campaign's freshness bounds want a slot
         * comfortably above zero. */
        let warp_slot = if real_pyth {
            #[cfg(feature = "non-production-real-pyth-lab")]
            {
                /* Agave 4.2.1's ProgramTest warp does not advance the program
                 * cache's latest-root marker. Root the router deployment
                 * exactly first; `initialize_real_pyth` persists a Verified
                 * VAA and only then advances to the receiver deployment. */
                REAL_PYTH_ROUTER_WARP_SLOT
            }
            #[cfg(not(feature = "non-production-real-pyth-lab"))]
            {
                unreachable!()
            }
        } else {
            WARP_SLOT
        };
        context.warp_to_slot(warp_slot).expect("warp");
        #[cfg(feature = "non-production-real-pyth-lab")]
        if real_pyth {
            context.set_sysvar(&Clock {
                slot: REAL_PYTH_ROUTER_WARP_SLOT,
                epoch_start_timestamp: REAL_PYTH_PUBLISH_TIME,
                epoch: 0,
                leader_schedule_epoch: 0,
                unix_timestamp: REAL_PYTH_PUBLISH_TIME + 240,
            });
        }

        let (slot, unix) = clock(&mut context).await;
        /* Place the whole window in the settled past: the last bucket's closing
         * boundary is at least two minutes behind the Clock, so every append's
         * boundary-plus-grace maturity check is satisfied, while the first
         * boundary is well inside the spec's 600-second staleness bound. */
        let (start_bucket, end_bucket_exclusive) = window.unwrap_or_else(|| {
            let end_bucket_exclusive = (unix as u64 - 120) / 60;
            (end_bucket_exclusive - SPAN, end_bucket_exclusive)
        });
        assert!(
            start_bucket > 0,
            "the bank clock must be past the epoch for a 60-second grid"
        );
        assert!(
            end_bucket_exclusive > start_bucket,
            "the pull window must contain at least one bucket"
        );

        let built = if window.is_some() {
            pull_plane(
                actor.pubkey(),
                spec,
                start_bucket,
                end_bucket_exclusive,
                kind,
            )
        } else {
            match kind {
                PullMarketKind::Occupation => {
                    pull_occupation_plane(actor.pubkey(), spec, start_bucket)
                }
                PullMarketKind::Categorical { .. } => {
                    pull_categorical_plane(actor.pubkey(), spec, start_bucket)
                }
            }
        };
        let plane = built.plane;

        for account in &plane.accounts {
            context.set_account(
                &account.address,
                &genesis_account(account.data.clone(), account.owner, false).into(),
            );
        }
        for (index, mint) in plane.outcome_mints.iter().enumerate() {
            let _ = index;
            context.set_account(
                &mint.address,
                &genesis_account(
                    outcome_mint_bytes(plane.market.address, 0),
                    TOKEN_2022,
                    false,
                )
                .into(),
            );
        }
        context.set_account(
            &COLLATERAL_MINT,
            &genesis_account(collateral_mint_bytes(SETS + DEPOSIT), TOKEN_2022, false).into(),
        );
        context.set_account(
            &ENDOW_OWNER_TOKEN,
            &genesis_account(
                token_account_bytes(COLLATERAL_MINT, endow_owner_keypair().pubkey(), DEPOSIT),
                TOKEN_2022,
                false,
            )
            .into(),
        );
        context.set_account(
            &ACTOR_TOKEN,
            &genesis_account(
                token_account_bytes(COLLATERAL_MINT, actor.pubkey(), 0),
                TOKEN_2022,
                false,
            )
            .into(),
        );
        context.set_account(
            &plane.hoard_token.address,
            &genesis_account(
                immutable_owner_account_bytes(COLLATERAL_MINT, plane.hoard_authority.address, SETS),
                TOKEN_2022,
                false,
            )
            .into(),
        );

        let _ = slot;
        Self {
            context,
            nonce: 0,
            actor,
            endow_owner: endow_owner_keypair(),
            update,
            decoy_update,
            #[cfg(feature = "non-production-real-pyth-lab")]
            encoded_vaa: encoded_vaa_keypair(),
            plane,
            spec: built.spec,
            start_bucket: built.start_bucket,
            end_bucket_exclusive: built.end_bucket_exclusive,
        }
    }

    /* -- instruction builders -------------------------------------------- */

    fn init_spec(&self) -> Instruction {
        self.init_spec_body(self.spec.encode_canonical())
    }

    fn init_spec_body(&self, body: [u8; 368]) -> Instruction {
        let data = layout_request(
            0,
            Intent::InitSourceSpecV2 {
                terms: self.plane.terms_id,
                spec_body: body,
            },
        );
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &data,
            vec![
                AccountMeta::new(self.actor.pubkey(), true),
                AccountMeta::new(self.plane.source_spec.address, false),
                AccountMeta::new(self.plane.feed.address, false),
                AccountMeta::new_readonly(self.plane.terms.address, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(RENT_SYSVAR, false),
            ],
        )
    }

    /// Take custody against the spec this family founded.
    fn endow(&self, amount: u64) -> Instruction {
        let (position, replay) = self.plane.owner_plane(self.endow_owner.pubkey());
        let metas = vec![
            AccountMeta::new(self.endow_owner.pubkey(), true),
            AccountMeta::new_readonly(self.plane.market.address, false),
            AccountMeta::new_readonly(self.plane.hoard.address, false),
            AccountMeta::new(position.address, false),
            AccountMeta::new(replay.address, false),
            AccountMeta::new_readonly(self.plane.profile.address, false),
            AccountMeta::new_readonly(self.plane.policy_account, false),
            AccountMeta::new_readonly(TOKEN_2022, false),
            AccountMeta::new_readonly(COLLATERAL_MINT, false),
            AccountMeta::new(ENDOW_OWNER_TOKEN, false),
            AccountMeta::new(self.plane.hoard_token.address, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(self.plane.terms.address, false),
            AccountMeta::new_readonly(self.plane.source_spec.address, false),
        ];
        assert_eq!(
            metas.len(),
            clutch_sbf::instructions::genesis::ENDOW_ACCOUNT_COUNT
        );
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::Endow {
                    market: self.plane.market_id,
                    owner: Hash32::from_bytes(self.endow_owner.pubkey().to_bytes()),
                    amount,
                },
            ),
            metas,
        )
    }

    fn init_archive(&self) -> Instruction {
        let data = layout_request(
            0,
            Intent::InitSourceArchiveV2 {
                terms: self.plane.terms_id,
            },
        );
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &data,
            vec![
                AccountMeta::new(self.actor.pubkey(), true),
                AccountMeta::new_readonly(self.plane.source_spec.address, false),
                AccountMeta::new_readonly(self.plane.feed.address, false),
                AccountMeta::new_readonly(self.plane.terms.address, false),
                AccountMeta::new(self.plane.source_archive.address, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(RENT_SYSVAR, false),
            ],
        )
    }

    /// The preceding instruction: a real write by the laboratory receiver,
    /// laid out as the reviewed seven-account Pyth `post_update` contract.
    fn post(&self, update: Address, body: &[u8]) -> Instruction {
        self.post_with(
            update,
            body,
            fixture::POST_UPDATE_DISCRIMINATOR,
            false,
            false,
        )
    }

    fn post_with(
        &self,
        update: Address,
        body: &[u8],
        discriminator: [u8; 8],
        authority_writable: bool,
        extra_account: bool,
    ) -> Instruction {
        assert_eq!(body.len(), PRICE_UPDATE_V2_ACCOUNT_LEN);
        let mut data = discriminator.to_vec();
        data.extend_from_slice(body);
        let authority = if authority_writable {
            AccountMeta::new(self.actor.pubkey(), true)
        } else {
            AccountMeta::new_readonly(self.actor.pubkey(), true)
        };
        let mut metas = vec![
            AccountMeta::new(self.context.payer.pubkey(), true),
            AccountMeta::new_readonly(Address::new_from_array([0x02; 32]), false),
            AccountMeta::new_readonly(Address::new_from_array(fixture::RECEIVER_CONFIG), false),
            AccountMeta::new(Address::new_from_array([0x04; 32]), false),
            AccountMeta::new(update, true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            authority,
        ];
        if extra_account {
            metas.push(AccountMeta::new_readonly(
                Address::new_from_array([0x08; 32]),
                false,
            ));
        }
        Instruction::new_with_bytes(
            Address::new_from_array(fixture::RECEIVER_PROGRAM),
            &data,
            metas,
        )
    }

    fn append(&self, sequence: u64, update: Address) -> Instruction {
        let data = layout_request(
            sequence,
            Intent::AppendSourceArchiveV2 {
                terms: self.plane.terms_id,
            },
        );
        self.append_with_archive(data, update, self.plane.source_archive.address, true)
    }

    fn append_readonly_update(&self, sequence: u64, update: Address) -> Instruction {
        let data = layout_request(
            sequence,
            Intent::AppendSourceArchiveV2 {
                terms: self.plane.terms_id,
            },
        );
        self.append_with_archive(data, update, self.plane.source_archive.address, false)
    }

    fn append_with_archive(
        &self,
        data: Vec<u8>,
        update: Address,
        archive: Address,
        update_writable: bool,
    ) -> Instruction {
        self.append_with_provider_config(
            data,
            update,
            archive,
            update_writable,
            Address::new_from_array(self.spec.fields().receiver_config),
        )
    }

    fn append_with_provider_config(
        &self,
        data: Vec<u8>,
        update: Address,
        archive: Address,
        update_writable: bool,
        receiver_config: Address,
    ) -> Instruction {
        let update = if update_writable {
            AccountMeta::new(update, false)
        } else {
            AccountMeta::new_readonly(update, false)
        };
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &data,
            vec![
                AccountMeta::new_readonly(self.plane.source_spec.address, false),
                AccountMeta::new_readonly(self.plane.feed.address, false),
                AccountMeta::new_readonly(self.plane.terms.address, false),
                AccountMeta::new(archive, false),
                AccountMeta::new_readonly(
                    Address::new_from_array(self.spec.fields().receiver_program),
                    false,
                ),
                AccountMeta::new_readonly(
                    Address::new_from_array(self.spec.fields().receiver_programdata),
                    false,
                ),
                AccountMeta::new_readonly(receiver_config, false),
                update,
                AccountMeta::new_readonly(INSTRUCTIONS_SYSVAR, false),
                AccountMeta::new_readonly(CLOCK_SYSVAR, false),
            ],
        )
    }

    #[cfg(feature = "non-production-real-pyth-lab")]
    async fn initialize_real_pyth(&mut self) {
        assert!(is_real_pyth_spec(self.spec));
        let router = Address::new_from_array(real_pyth_lab::ROUTER_PROGRAM);
        let payer = self.context.payer.pubkey();
        let router_config = Address::find_program_address(&[b"Bridge"], &router).0;
        let guardian_set =
            Address::find_program_address(&[b"GuardianSet", &0_u32.to_be_bytes()], &router).0;
        let fee_collector = Address::find_program_address(&[b"fee_collector"], &router).0;
        let router_initialize = Instruction::new_with_bytes(
            router,
            &real_pyth_fixture("router-initialize.data"),
            vec![
                AccountMeta::new(router_config, false),
                AccountMeta::new(guardian_set, false),
                AccountMeta::new(fee_collector, false),
                AccountMeta::new(payer, true),
                AccountMeta::new_readonly(CLOCK_SYSVAR, false),
                AccountMeta::new_readonly(RENT_SYSVAR, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            ],
        );
        assert_eq!(self.send(router_initialize).await.0, Ok(()));

        const VAA_START: usize = 46;
        const WRITE_SPLIT: usize = 755;
        const INIT_ENCODED_VAA: [u8; 8] = [209, 193, 173, 25, 91, 202, 181, 218];
        const WRITE_ENCODED_VAA: [u8; 8] = [199, 208, 110, 177, 150, 76, 118, 42];
        const VERIFY_ENCODED_VAA_V1: [u8; 8] = [103, 56, 177, 229, 240, 103, 68, 73];
        let vaa = real_pyth_fixture("signed.vaa");
        let encoded = self.encoded_vaa.pubkey();
        let create = system_instruction::create_account(
            &payer,
            &encoded,
            Rent::default().minimum_balance(VAA_START + vaa.len()),
            (VAA_START + vaa.len()) as u64,
            &router,
        );
        let init = Instruction::new_with_bytes(
            router,
            &INIT_ENCODED_VAA,
            vec![
                AccountMeta::new_readonly(payer, true),
                AccountMeta::new(encoded, false),
            ],
        );
        let write_instruction = |index: usize, bytes: &[u8]| {
            let mut data = WRITE_ENCODED_VAA.to_vec();
            data.extend_from_slice(&(index as u32).to_le_bytes());
            data.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            data.extend_from_slice(bytes);
            Instruction::new_with_bytes(
                router,
                &data,
                vec![
                    AccountMeta::new_readonly(payer, true),
                    AccountMeta::new(encoded, false),
                ],
            )
        };
        let split = vaa.len().min(WRITE_SPLIT);
        assert_eq!(
            self.send_many(&[create, init, write_instruction(0, &vaa[..split])])
                .await
                .0,
            Ok(())
        );
        let verify = Instruction::new_with_bytes(
            router,
            &VERIFY_ENCODED_VAA_V1,
            vec![
                AccountMeta::new_readonly(payer, true),
                AccountMeta::new(encoded, false),
                AccountMeta::new_readonly(guardian_set, false),
            ],
        );
        let mut final_verify = Vec::new();
        if split < vaa.len() {
            final_verify.push(write_instruction(split, &vaa[split..]));
        }
        final_verify.push(verify);
        assert_eq!(self.send_many(&final_verify).await.0, Ok(()));
        assert_eq!(
            self.data(encoded).await[8],
            2,
            "real router must mark the quorum-signed VAA Verified"
        );

        /* ProgramTest 4.2.1 does not update ProgramCache.latest_root_slot in
         * `warp_to_slot`. Keeping the router's exact deployment slot visible
         * therefore requires verifying and persisting the VAA at its own
         * D+1 root before advancing to the receiver's D+1 root. No router
         * instruction occurs after this second warp. */
        self.context
            .warp_to_slot(REAL_PYTH_WARP_SLOT)
            .expect("warp from router generation to receiver generation");
        self.context.set_sysvar(&Clock {
            slot: REAL_PYTH_WARP_SLOT,
            epoch_start_timestamp: REAL_PYTH_PUBLISH_TIME,
            epoch: 0,
            leader_schedule_epoch: 0,
            unix_timestamp: REAL_PYTH_PUBLISH_TIME + 240,
        });

        let receiver = Address::new_from_array(real_pyth_lab::RECEIVER_PROGRAM);
        let receiver_initialize = Instruction::new_with_bytes(
            receiver,
            &real_pyth_fixture("receiver-initialize.data"),
            vec![
                AccountMeta::new(payer, true),
                AccountMeta::new(
                    Address::new_from_array(real_pyth_lab::RECEIVER_CONFIG),
                    false,
                ),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            ],
        );
        assert_eq!(self.send(receiver_initialize).await.0, Ok(()));
        assert_eq!(
            self.data(Address::new_from_array(real_pyth_lab::RECEIVER_CONFIG))
                .await,
            real_pyth_fixture("receiver-config.account"),
            "real receiver must write the exact locally pinned Config body"
        );
    }

    #[cfg(feature = "non-production-real-pyth-lab")]
    fn real_pyth_post(&self, update: Address) -> Instruction {
        let receiver = Address::new_from_array(real_pyth_lab::RECEIVER_PROGRAM);
        let treasury = Address::find_program_address(&[b"treasury", &[0]], &receiver).0;
        let payer = self.context.payer.pubkey();
        Instruction::new_with_bytes(
            receiver,
            &real_pyth_fixture("receiver-post-update.data"),
            vec![
                AccountMeta::new(payer, true),
                AccountMeta::new_readonly(self.encoded_vaa.pubkey(), false),
                AccountMeta::new_readonly(
                    Address::new_from_array(real_pyth_lab::RECEIVER_CONFIG),
                    false,
                ),
                AccountMeta::new(treasury, false),
                AccountMeta::new(update, true),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(payer, true),
            ],
        )
    }

    #[cfg(feature = "non-production-real-pyth-lab")]
    fn append_with_config(&self, sequence: u64, update: Address, config: Address) -> Instruction {
        self.append_with_provider_config(
            layout_request(
                sequence,
                Intent::AppendSourceArchiveV2 {
                    terms: self.plane.terms_id,
                },
            ),
            update,
            self.plane.source_archive.address,
            true,
            config,
        )
    }

    fn seal(&self, sequence: u64) -> Instruction {
        let data = layout_request(
            sequence,
            Intent::SealSourceArchiveV2 {
                terms: self.plane.terms_id,
            },
        );
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &data,
            vec![
                AccountMeta::new_readonly(self.plane.source_spec.address, false),
                AccountMeta::new(self.plane.feed.address, false),
                AccountMeta::new_readonly(self.plane.terms.address, false),
                AccountMeta::new(self.plane.source_archive.address, false),
            ],
        )
    }

    fn resolve(&self) -> Instruction {
        self.resolve_with_payout_and_archive(
            PAYOUT_INDEX_UNRESOLVED,
            self.plane.source_archive.address,
        )
    }

    fn resolve_with_payout(&self, payout_index: u8) -> Instruction {
        self.resolve_with_payout_and_archive(payout_index, self.plane.source_archive.address)
    }

    fn resolve_with_payout_and_buffer(&self, payout_index: u8, buffer: Address) -> Instruction {
        let mut data = vec![0xd1, 1];
        data.extend_from_slice(&0_u64.to_le_bytes());
        data.push(1);
        data.push(payout_index);
        let mut metas = vec![
            AccountMeta::new_readonly(self.actor.pubkey(), true),
            AccountMeta::new(self.plane.market.address, false),
            AccountMeta::new_readonly(self.plane.hoard.address, false),
            AccountMeta::new(self.plane.kernel.address, false),
            AccountMeta::new(self.plane.supply.address, false),
            AccountMeta::new_readonly(self.plane.terms.address, false),
            AccountMeta::new(self.plane.resolution.address, false),
            AccountMeta::new_readonly(self.plane.feed.address, false),
            AccountMeta::new_readonly(self.plane.source_spec.address, false),
            AccountMeta::new_readonly(self.plane.source_archive.address, false),
            AccountMeta::new_readonly(buffer, false),
        ];
        metas.extend(
            self.plane
                .outcome_mints
                .iter()
                .map(|mint| AccountMeta::new_readonly(mint.address, false)),
        );
        assert_eq!(
            metas.len(),
            observe_resolve::RESOLVE_ACCOUNT_PREFIX + usize::from(OUTCOMES)
        );
        Instruction::new_with_bytes(PROGRAM_ID, &data, metas)
    }

    fn resolve_with_payout_and_archive(&self, payout_index: u8, archive: Address) -> Instruction {
        let mut data = vec![0xd1, 1];
        data.extend_from_slice(&0_u64.to_le_bytes());
        data.push(1);
        data.push(payout_index);
        let mut metas = vec![
            AccountMeta::new_readonly(self.actor.pubkey(), true),
            AccountMeta::new(self.plane.market.address, false),
            AccountMeta::new_readonly(self.plane.hoard.address, false),
            AccountMeta::new(self.plane.kernel.address, false),
            AccountMeta::new(self.plane.supply.address, false),
            AccountMeta::new_readonly(self.plane.terms.address, false),
            AccountMeta::new(self.plane.resolution.address, false),
            AccountMeta::new_readonly(self.plane.feed.address, false),
            AccountMeta::new_readonly(self.plane.source_spec.address, false),
            AccountMeta::new_readonly(archive, false),
        ];
        metas.extend(
            self.plane
                .outcome_mints
                .iter()
                .map(|mint| AccountMeta::new_readonly(mint.address, false)),
        );
        assert_eq!(
            metas.len(),
            observe_resolve::ARCHIVE_DIRECT_RESOLVE_ACCOUNT_PREFIX + usize::from(OUTCOMES)
        );
        Instruction::new_with_bytes(PROGRAM_ID, &data, metas)
    }

    fn redeem(&self, sequence: u64, outcome: u8, quantity: u64) -> Instruction {
        let mut data = vec![0xd1, 1];
        data.extend_from_slice(&sequence.to_le_bytes());
        data.push(2);
        data.push(outcome);
        data.extend_from_slice(&quantity.to_le_bytes());
        let mut metas = vec![
            AccountMeta::new_readonly(self.actor.pubkey(), true),
            AccountMeta::new(self.plane.market.address, false),
            AccountMeta::new(self.plane.hoard.address, false),
            AccountMeta::new(self.plane.position.address, false),
            AccountMeta::new(self.plane.kernel.address, false),
            AccountMeta::new(self.plane.replay.address, false),
            AccountMeta::new(self.plane.supply.address, false),
            AccountMeta::new_readonly(self.plane.terms.address, false),
            AccountMeta::new_readonly(self.plane.resolution.address, false),
            AccountMeta::new_readonly(self.plane.profile.address, false),
            AccountMeta::new_readonly(TOKEN_2022, false),
            AccountMeta::new_readonly(self.plane.policy_account, false),
            AccountMeta::new_readonly(COLLATERAL_MINT, false),
            AccountMeta::new(ACTOR_TOKEN, false),
            AccountMeta::new_readonly(self.plane.hoard_authority.address, false),
            AccountMeta::new(self.plane.hoard_token.address, false),
        ];
        metas.extend(
            self.plane
                .outcome_mints
                .iter()
                .map(|mint| AccountMeta::new_readonly(mint.address, false)),
        );
        assert_eq!(
            metas.len(),
            observe_resolve::REDEEM_ACCOUNT_PREFIX + usize::from(OUTCOMES)
        );
        Instruction::new_with_bytes(PROGRAM_ID, &data, metas)
    }

    /* -- driving ---------------------------------------------------------- */

    /// Have the preceding receiver instruction write the ephemeral update,
    /// then consume it in the adjacent append.
    async fn ingest(&mut self, index: u64) -> (Result<(), TransactionError>, u64) {
        self.ingest_price(index, PRICE_ATOMS, 0).await
    }

    async fn ingest_price(
        &mut self,
        index: u64,
        price: i64,
        confidence: u64,
    ) -> (Result<(), TransactionError>, u64) {
        let bucket = self.start_bucket + index;
        let body = self
            .price_update_body_with(
                bucket,
                self.spec.fields().provider_feed_id,
                price,
                confidence,
            )
            .await;
        let update = self.update.pubkey();
        self.ingest_now(index, update, &body).await
    }

    /// Drive one receiver-write/append pair with caller-selected update bytes.
    async fn ingest_now(
        &mut self,
        index: u64,
        update: Address,
        body: &[u8],
    ) -> (Result<(), TransactionError>, u64) {
        let post = self.post(update, body);
        let append = self.append(index, update);
        self.send_many(&[post, append]).await
    }

    async fn price_update_body(&mut self, bucket: u64, feed_id: [u8; 32]) -> Vec<u8> {
        self.price_update_body_with(bucket, feed_id, PRICE_ATOMS, 0)
            .await
    }

    async fn price_update_body_with(
        &mut self,
        bucket: u64,
        feed_id: [u8; 32],
        price: i64,
        confidence: u64,
    ) -> Vec<u8> {
        let (slot, _) = clock(&mut self.context).await;
        let boundary = (bucket + 1) as i64 * 60;
        let update = PriceUpdateFixture {
            write_authority: self.actor.pubkey().to_bytes(),
            verification_level: 1,
            feed_id,
            price,
            confidence,
            exponent: -8,
            publish_time: boundary,
            prev_publish_time: boundary - 1,
            ema_price: price,
            ema_confidence: confidence,
            posted_slot: slot,
            trailing_pad: 0,
        };
        let data = price_update_body(update);
        assert_eq!(data.len(), PRICE_UPDATE_V2_ACCOUNT_LEN);
        data
    }

    async fn send(&mut self, instruction: Instruction) -> (Result<(), TransactionError>, u64) {
        self.send_many(&[instruction]).await
    }

    async fn send_many(
        &mut self,
        instructions: &[Instruction],
    ) -> (Result<(), TransactionError>, u64) {
        let blockhash = self
            .context
            .banks_client
            .get_latest_blockhash()
            .await
            .unwrap();
        self.nonce += 1;
        let mut all = vec![Instruction::new_with_bytes(
            COMPUTE_BUDGET,
            &compute_unit_limit_data(CU_LIMIT - self.nonce),
            Vec::new(),
        )];
        all.extend_from_slice(instructions);
        let payer = self.context.payer.insecure_clone();
        /* Only some of these instructions name the actor at all — an append
         * and a seal are permissionless and carry no signer — so the signer
         * set is read off the built message rather than assumed.  Offering a
         * key the message does not require is a `KeypairPubkeyMismatch`. */
        let mut transaction = Transaction::new_with_payer(&all, Some(&payer.pubkey()));
        let required = usize::from(transaction.message.header.num_required_signatures);
        let names: Vec<Address> = transaction.message.account_keys[..required].to_vec();
        let mut signers: Vec<&Keypair> = Vec::new();
        for key in &names {
            if *key == payer.pubkey() {
                signers.push(&payer);
            } else if *key == self.actor.pubkey() {
                signers.push(&self.actor);
            } else if *key == self.endow_owner.pubkey() {
                signers.push(&self.endow_owner);
            } else if *key == self.update.pubkey() {
                signers.push(&self.update);
            } else if *key == self.decoy_update.pubkey() {
                signers.push(&self.decoy_update);
            } else {
                #[cfg(feature = "non-production-real-pyth-lab")]
                if *key == self.encoded_vaa.pubkey() {
                    signers.push(&self.encoded_vaa);
                    continue;
                }
                panic!("no keypair for required signer {key}");
            }
        }
        transaction.sign(&signers, blockhash);
        let meta = self
            .context
            .banks_client
            .process_transaction_with_metadata(transaction)
            .await
            .expect("bank responds");
        let units = meta
            .metadata
            .as_ref()
            .map(|m| m.compute_units_consumed)
            .unwrap_or(0);
        (meta.result, units)
    }

    async fn data(&mut self, address: Address) -> Vec<u8> {
        self.context
            .banks_client
            .get_account(address)
            .await
            .unwrap()
            .unwrap_or_else(|| panic!("{address} must exist"))
            .data
    }

    async fn token_amount(&mut self, address: Address) -> u64 {
        let data = self.data(address).await;
        let mut amount = [0_u8; 8];
        amount.copy_from_slice(&data[64..72]);
        u64::from_le_bytes(amount)
    }

    async fn maybe_account(&mut self, address: Address) -> Option<Account> {
        self.context
            .banks_client
            .get_account(address)
            .await
            .unwrap()
    }
}

async fn clock(context: &mut ProgramTestContext) -> (u64, i64) {
    let account = context
        .banks_client
        .get_account(CLOCK_SYSVAR)
        .await
        .unwrap()
        .expect("clock sysvar exists");
    let mut slot = [0_u8; 8];
    slot.copy_from_slice(&account.data[..8]);
    let mut unix = [0_u8; 8];
    unix.copy_from_slice(&account.data[32..40]);
    (u64::from_le_bytes(slot), i64::from_le_bytes(unix))
}

fn collateral_mint_bytes(supply: u64) -> Vec<u8> {
    let mut out = vec![0_u8; 82];
    out[36..44].copy_from_slice(&supply.to_le_bytes());
    out[44] = 6;
    out[45] = 1;
    out
}

const CLOCK_SYSVAR: Address = Address::new_from_array(clutch_sbf::source_identity::CLOCK_SYSVAR_ID);
const INSTRUCTIONS_SYSVAR: Address =
    Address::new_from_array(clutch_sbf::instructions_sysvar::INSTRUCTIONS_SYSVAR_ID);

async fn found_ingested_sealed(
    campaign: &mut Campaign,
    price: i64,
    confidence: u64,
) -> (u64, u64, u64, u64) {
    let init_spec = campaign.init_spec();
    let (result, spec_cu) = campaign.send(init_spec).await;
    assert_eq!(result, Ok(()), "InitSourceSpecV2 must be accepted");

    let init_archive = campaign.init_archive();
    let (result, archive_cu) = campaign.send(init_archive).await;
    assert_eq!(result, Ok(()), "InitSourceArchiveV2 must be accepted");

    let mut append_cu = 0;
    for index in 0..SPAN {
        let (result, units) = campaign.ingest_price(index, price, confidence).await;
        assert_eq!(result, Ok(()), "append {index} must be accepted");
        append_cu = units;
    }

    let seal = campaign.seal(SPAN);
    let (result, seal_cu) = campaign.send(seal).await;
    assert_eq!(result, Ok(()), "SealSourceArchiveV2 must be accepted");
    (spec_cu, archive_cu, append_cu, seal_cu)
}

async fn resolve_plane_images(campaign: &mut Campaign) -> Vec<(Address, Vec<u8>)> {
    let mut addresses = vec![
        campaign.plane.market.address,
        campaign.plane.hoard.address,
        campaign.plane.kernel.address,
        campaign.plane.supply.address,
        campaign.plane.terms.address,
        campaign.plane.resolution.address,
        campaign.plane.feed.address,
        campaign.plane.source_spec.address,
        campaign.plane.source_archive.address,
    ];
    addresses.extend(campaign.plane.outcome_mints.iter().map(|mint| mint.address));
    let mut images = Vec::with_capacity(addresses.len());
    for address in addresses {
        images.push((address, campaign.data(address).await));
    }
    images
}

/* ------------------------------------------------------------------------ */
/* The circle                                                                */
/* ------------------------------------------------------------------------ */

#[tokio::test]
async fn the_default_elf_founds_ingests_seals_and_resolves_a_v2_window() {
    let mut campaign = Campaign::start(registered_spec()).await;

    // 1. Found the spec and its feed head (tag 70).
    assert!(
        campaign
            .maybe_account(campaign.plane.source_spec.address)
            .await
            .is_none(),
        "the spec must not exist before its own instruction creates it"
    );
    let (result, spec_cu) = campaign.send(campaign.init_spec()).await;
    assert_eq!(result, Ok(()), "InitSourceSpecV2 must be accepted");
    let spec_account = campaign.data(campaign.plane.source_spec.address).await;
    assert_eq!(spec_account.len(), SOURCE_SPEC_ACCOUNT_V2_BYTES);
    let feed_account = campaign.data(campaign.plane.feed.address).await;
    let feed = FeedAccount::decode(&feed_account).expect("feed head decodes");
    assert_eq!(feed.feed, campaign.plane.feed_id);
    assert_eq!(feed.cursor, campaign.start_bucket);
    assert_eq!(feed.archive_pages, 0);

    // 2. Found the archive page (tag 71).
    let (result, archive_cu) = campaign.send(campaign.init_archive()).await;
    assert_eq!(result, Ok(()), "InitSourceArchiveV2 must be accepted");
    let page = campaign.data(campaign.plane.source_archive.address).await;
    assert_eq!(page.len(), SOURCE_ARCHIVE_ACCOUNT_V2_BYTES);

    // 3. Ingest the whole window, one authenticated pull record per boundary
    //    (tag 72), each behind a real receiver post.
    let mut append_cu = 0;
    for index in 0..SPAN {
        let (result, units) = campaign.ingest(index).await;
        assert_eq!(result, Ok(()), "append {index} must be accepted");
        append_cu = units;
    }

    // 4. Seal (tag 73).
    let (result, seal_cu) = campaign.send(campaign.seal(SPAN)).await;
    assert_eq!(result, Ok(()), "SealSourceArchiveV2 must be accepted");
    let feed = FeedAccount::decode(&campaign.data(campaign.plane.feed.address).await)
        .expect("sealed feed decodes");
    assert_eq!(
        feed.cursor, campaign.end_bucket_exclusive,
        "a v2 seal advances the head to the window end, not the maturity bucket"
    );
    assert_eq!(feed.archive_pages, 1);

    // 5. The market resolves from the page the program built.
    let (result, resolve_cu) = campaign.send(campaign.resolve()).await;
    assert_eq!(result, Ok(()), "Resolve must read the v2 page");
    let resolution = OccupationResolutionAccount::decode(
        &campaign.data(campaign.plane.resolution.address).await,
    )
    .expect("occupation resolution decodes");
    assert_eq!(
        resolution.mode,
        RESOLUTION_MODE_DERIVED_QUANTIZED_OCCUPATION
    );
    assert_eq!(resolution.sample_count, SPAN);
    assert_eq!(resolution.coverage_count, SPAN);
    assert_eq!(resolution.vector.denominator, DENOMINATOR);
    /* The exact vector a V1 archive of the same three point observations
     * produces at degree one. The generations agree because the fold is one
     * fold; they are not interchangeable because the page tags and commitment
     * domains are disjoint. */
    let mut expected = [0_u64; MAX_OUTCOMES];
    expected[..4].copy_from_slice(&[32, 32, 0, 0]);
    assert_eq!(resolution.vector.weights, expected);

    // 6. Redemption pays, and the market conserves.
    let hoard_before = HoardAccount::decode(&campaign.data(campaign.plane.hoard.address).await)
        .expect("hoard decodes");
    let position_before =
        PositionAccount::decode(&campaign.data(campaign.plane.position.address).await)
            .expect("position decodes");
    let supply_before =
        SupplyLedgerAccount::decode(&campaign.data(campaign.plane.supply.address).await)
            .expect("supply decodes");
    let token_before = campaign.token_amount(ACTOR_TOKEN).await;

    let (result, redeem_cu) = campaign.send(campaign.redeem(0, 0, 2)).await;
    assert_eq!(result, Ok(()), "internal redemption must be paid");

    let hoard_after = HoardAccount::decode(&campaign.data(campaign.plane.hoard.address).await)
        .expect("hoard decodes");
    let position_after =
        PositionAccount::decode(&campaign.data(campaign.plane.position.address).await)
            .expect("position decodes");
    let supply_after =
        SupplyLedgerAccount::decode(&campaign.data(campaign.plane.supply.address).await)
            .expect("supply decodes");
    let token_after = campaign.token_amount(ACTOR_TOKEN).await;

    let paid = position_after.cash_atoms - position_before.cash_atoms;
    assert!(paid > 0, "the holder must actually be paid");
    assert_eq!(
        token_after, token_before,
        "an internal redemption moves no Token-2022 atoms"
    );
    assert_eq!(
        position_before.internal[0] - position_after.internal[0],
        2,
        "exactly the burned claims leave the position"
    );
    assert_eq!(
        supply_before.internal_supply[0] - supply_after.internal_supply[0],
        2,
        "the market-wide ledger burns the same claims"
    );
    /* Conservation across the value boundary. An internal redemption converts
     * set-backing collateral into an internal cash claim: the Hoard's
     * set-backing total falls by exactly the atoms the holder's cash rose by,
     * and the real Token-2022 balance the Hoard custodies does not move at all,
     * because nothing left the market. Nothing was created by resolving from a
     * v2 page rather than a V1 one. */
    assert_eq!(
        hoard_before.collateral_atoms - hoard_after.collateral_atoms,
        paid,
        "the Hoard's set-backing falls by exactly what the holder is paid"
    );
    assert_eq!(
        campaign
            .token_amount(campaign.plane.hoard_token.address)
            .await,
        SETS,
        "the custodied Token-2022 balance is untouched by an internal redemption"
    );

    println!(
        "r2 v2 wire CU: init_spec={spec_cu} init_archive={archive_cu} \
         append={append_cu} seal={seal_cu} resolve={resolve_cu} redeem={redeem_cu}"
    );
}

#[tokio::test]
async fn degree_zero_v2_nonzero_confidence_same_cell_resolves_without_buffer() {
    let mut campaign = Campaign::start_categorical(registered_spec()).await;
    let (spec_cu, archive_cu, append_cu, seal_cu) =
        found_ingested_sealed(&mut campaign, 400, 1).await;

    let resolve = campaign.resolve_with_payout(0);
    assert_eq!(
        resolve.accounts.len(),
        observe_resolve::ARCHIVE_DIRECT_RESOLVE_ACCOUNT_PREFIX + usize::from(OUTCOMES),
        "degree-zero v2 categorical Resolve uses the no-buffer account shape"
    );
    let (result, resolve_cu) = campaign.send(resolve).await;
    assert_eq!(
        result,
        Ok(()),
        "the authenticated [397,403] interval lies wholly in cell 0"
    );

    let resolution =
        ResolutionAccount::decode(&campaign.data(campaign.plane.resolution.address).await)
            .expect("categorical resolution decodes");
    assert_eq!(resolution.payout_index, 0);
    assert_eq!(
        resolution.sealed_end_bucket_exclusive,
        campaign.end_bucket_exclusive
    );
    assert_eq!(resolution.feed_cursor, campaign.end_bucket_exclusive);
    assert_eq!(resolution.repair_generation, 0);

    let market =
        MarketAccount::decode(&campaign.data(campaign.plane.market.address).await).expect("market");
    assert_eq!(market.lifecycle, 1, "the market is recorded resolved");

    println!(
        "r2 v2 categorical CU: init_spec={spec_cu} init_archive={archive_cu} \
         append={append_cu} seal={seal_cu} resolve_same_cell={resolve_cu} \
         resolve_accounts={}",
        observe_resolve::ARCHIVE_DIRECT_RESOLVE_ACCOUNT_PREFIX + usize::from(OUTCOMES)
    );
}

#[tokio::test]
async fn degree_zero_v2_boundary_straddle_refuses_atomically_without_buffer() {
    let mut campaign = Campaign::start_categorical(registered_spec()).await;
    let (_spec_cu, _archive_cu, append_cu, seal_cu) =
        found_ingested_sealed(&mut campaign, 500, 1).await;
    let before = resolve_plane_images(&mut campaign).await;

    let resolve = campaign.resolve_with_payout(0);
    assert_eq!(
        resolve.accounts.len(),
        observe_resolve::ARCHIVE_DIRECT_RESOLVE_ACCOUNT_PREFIX + usize::from(OUTCOMES),
        "the refusing categorical case also uses the no-buffer account shape"
    );
    let (result, refuse_cu) = campaign.send(resolve).await;
    assert_eq!(
        custom(&result),
        RESOLUTION_REFUSAL,
        "the authenticated [497,503] interval crosses the 500 boundary"
    );
    assert_eq!(
        resolve_plane_images(&mut campaign).await,
        before,
        "the boundary-straddle refusal atomically preserves every Resolve-plane account image"
    );
    let resolution =
        ResolutionAccount::decode(&campaign.data(campaign.plane.resolution.address).await)
            .expect("categorical resolution decodes");
    assert_eq!(resolution.payout_index, PAYOUT_INDEX_UNRESOLVED);

    println!(
        "r2 v2 categorical CU: append={append_cu} seal={seal_cu} \
         resolve_boundary_refusal={refuse_cu} resolve_accounts={}",
        observe_resolve::ARCHIVE_DIRECT_RESOLVE_ACCOUNT_PREFIX + usize::from(OUTCOMES)
    );
}

#[tokio::test]
async fn degree_zero_v2_rejects_legacy_buffer_shape_atomically() {
    let mut campaign = Campaign::start_categorical(registered_spec()).await;
    let (_spec_cu, _archive_cu, append_cu, seal_cu) =
        found_ingested_sealed(&mut campaign, 400, 1).await;
    let before = resolve_plane_images(&mut campaign).await;

    let resolve = campaign.resolve_with_payout_and_buffer(0, RENT_SYSVAR);
    assert_eq!(
        resolve.accounts.len(),
        observe_resolve::RESOLVE_ACCOUNT_PREFIX + usize::from(OUTCOMES),
        "the legacy buffer account shape is 11+n"
    );
    let (result, refuse_cu) = campaign.send(resolve).await;
    assert_eq!(
        custom(&result),
        ClutchError::AccountCount as u32,
        "after authenticating the v2 archive generation, degree-zero categorical expects 10+n"
    );
    assert_eq!(
        resolve_plane_images(&mut campaign).await,
        before,
        "the shape refusal preserves every Resolve-plane account image"
    );

    println!(
        "r2 v2 categorical CU: append={append_cu} seal={seal_cu} \
         resolve_legacy_buffer_shape_refusal={refuse_cu} resolve_accounts={}",
        observe_resolve::RESOLVE_ACCOUNT_PREFIX + usize::from(OUTCOMES)
    );
}

/* ------------------------------------------------------------------------ */
/* The hostile battery                                                       */
/* ------------------------------------------------------------------------ */

/// Refusal codes this battery pins, by name.
const SOURCE_RELEASE_UNAVAILABLE: u32 = 0x0079;
const SOURCE_ADMISSION_FAILED: u32 = 0x007a;
const NOT_WRITABLE: u32 = 0x0005;
const ALREADY_INITIALIZED: u32 = 0x0040;
const REPLAY: u32 = 0x000d;
const WRONG_PROGRAM_OWNER: u32 = 0x0004;
const RESOLUTION_EVIDENCE_UNAVAILABLE: u32 = 0x0010;
const RESOLUTION_REFUSAL: u32 = 0x0051;

fn custom(result: &Result<(), TransactionError>) -> u32 {
    match result {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(code))) => *code,
        other => panic!("expected a custom-code refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn founding_a_v2_spec_refuses_every_hostile_shape_without_writing() {
    let mut campaign = Campaign::start(registered_spec()).await;

    assert!(
        campaign
            .maybe_account(campaign.plane.source_spec.address)
            .await
            .is_none(),
        "nothing exists before the founding instruction runs"
    );

    // A V1 spec body padded to the v2 width: the generations do not decode as
    // one another even at equal length.
    let mut v1_shaped = [0_u8; 368];
    v1_shaped[..8].copy_from_slice(b"DCSRCV1\0");
    let (result, _) = campaign.send(campaign.init_spec_body(v1_shaped)).await;
    assert_eq!(custom(&result), SOURCE_ADMISSION_FAILED);

    // A registered release naming a different asset pair: a valid spec whose
    // own identity is not the feed this market's Terms froze.
    let mut other = pull_spec_fields();
    other.base_asset_id = [0x9a; 32];
    let other = SourceSpecV2::new(other).expect("still a valid v2 spec");
    assert_ne!(other.feed_id(), campaign.spec.feed_id());
    let (result, _) = campaign
        .send(campaign.init_spec_body(other.encode_canonical()))
        .await;
    assert_eq!(custom(&result), SOURCE_ADMISSION_FAILED);

    // The honest founding lands.
    let (result, _) = campaign.send(campaign.init_spec()).await;
    assert_eq!(result, Ok(()));
    let founded = campaign.data(campaign.plane.source_spec.address).await;

    // And is not re-foundable.
    let (result, _) = campaign.send(campaign.init_spec()).await;
    assert_eq!(custom(&result), ALREADY_INITIALIZED);
    assert_eq!(
        campaign.data(campaign.plane.source_spec.address).await,
        founded,
        "a refused re-founding leaves the account byte-identical"
    );
}

#[tokio::test]
async fn an_append_refuses_without_the_exact_adjacent_receiver_post() {
    let mut campaign = Campaign::start(registered_spec()).await;
    assert_eq!(campaign.send(campaign.init_spec()).await.0, Ok(()));
    assert_eq!(campaign.send(campaign.init_archive()).await.0, Ok(()));
    let genesis_page = campaign.data(campaign.plane.source_archive.address).await;

    let bucket = campaign.start_bucket;
    let update = campaign.update.pubkey();
    let decoy = campaign.decoy_update.pubkey();
    let honest_body = campaign
        .price_update_body(bucket, fixture::PROVIDER_FEED_ID)
        .await;

    // The consumer contract names the real transaction-effective privilege:
    // a standalone readonly update refuses before any source parsing.
    let (result, _) = campaign
        .send(campaign.append_readonly_update(0, update))
        .await;
    assert_eq!(custom(&result), NOT_WRITABLE);

    // No post at all: the preceding instruction is the compute-budget one.
    let (result, _) = campaign.send(campaign.append(0, update)).await;
    assert_eq!(custom(&result), SOURCE_ADMISSION_FAILED);

    // A post to some other program.
    let wrong_program = Instruction::new_with_bytes(
        SYSTEM_PROGRAM,
        &[2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        vec![
            AccountMeta::new(campaign.actor.pubkey(), true),
            AccountMeta::new(Address::new_from_array([0x77; 32]), false),
        ],
    );
    let (result, _) = campaign
        .send_many(&[wrong_program, campaign.append(0, update)])
        .await;
    assert_eq!(custom(&result), SOURCE_ADMISSION_FAILED);

    // A post that names a *different* update account than the one presented.
    let (result, _) = campaign
        .send_many(&[
            campaign.post(decoy, &honest_body),
            campaign.append(0, update),
        ])
        .await;
    assert_eq!(custom(&result), SOURCE_ADMISSION_FAILED);

    // The lab receiver really writes a wrong-feed update before Dragon
    // refuses it. The transaction must roll both the receiver write and the
    // archive mutation back, which a no-op receiver could never prove.
    let update_before = campaign.data(update).await;
    assert_eq!(update_before, vec![0_u8; PRICE_UPDATE_V2_ACCOUNT_LEN]);
    let wrong_feed = campaign.price_update_body(bucket, [0x5f; 32]).await;
    let (result, _) = campaign.ingest_now(0, update, &wrong_feed).await;
    assert_eq!(custom(&result), SOURCE_ADMISSION_FAILED);
    assert_eq!(
        campaign.data(update).await,
        update_before,
        "a later append refusal atomically rolls back the receiver write"
    );

    // The same non-vacuous rollback check reaches each newly authenticated
    // post field: discriminator, exact account count, and a representative
    // effective flag. The lab writer deliberately accepts these shapes so the
    // refusal is Dragon's rather than the fixture's.
    let mut wrong_discriminator = fixture::POST_UPDATE_DISCRIMINATOR;
    wrong_discriminator[0] ^= 1;
    for (label, post) in [
        (
            "discriminator",
            campaign.post_with(update, &honest_body, wrong_discriminator, false, false),
        ),
        (
            "account count",
            campaign.post_with(
                update,
                &honest_body,
                fixture::POST_UPDATE_DISCRIMINATOR,
                false,
                true,
            ),
        ),
        (
            "write-authority writable flag",
            campaign.post_with(
                update,
                &honest_body,
                fixture::POST_UPDATE_DISCRIMINATOR,
                true,
                false,
            ),
        ),
    ] {
        let (result, _) = campaign
            .send_many(&[post, campaign.append(0, update)])
            .await;
        assert_eq!(custom(&result), SOURCE_ADMISSION_FAILED, "{label}");
        assert_eq!(
            campaign.data(update).await,
            update_before,
            "{label} rollback"
        );
        assert_eq!(
            campaign.data(campaign.plane.source_archive.address).await,
            genesis_page,
            "{label} archive rollback"
        );
    }

    assert_eq!(
        campaign.data(campaign.plane.source_archive.address).await,
        genesis_page,
        "every refused append leaves the page byte-identical"
    );

    // The honest append lands, and does not land twice.
    let (result, _) = campaign.ingest(0).await;
    assert_eq!(result, Ok(()));
    assert_eq!(
        campaign.data(update).await,
        honest_body,
        "the accepted evidence was written by the preceding receiver call"
    );
    let one_record = campaign.data(campaign.plane.source_archive.address).await;
    assert_ne!(one_record, genesis_page);
    let (result, _) = campaign.ingest(0).await;
    assert_eq!(
        custom(&result),
        REPLAY,
        "a replayed append is refused on the page's own record count"
    );
    assert_eq!(
        campaign.data(campaign.plane.source_archive.address).await,
        one_record,
        "the replay leaves the page byte-identical"
    );

    // A short page cannot be sealed.
    let (result, _) = campaign.send(campaign.seal(1)).await;
    assert_eq!(custom(&result), SOURCE_ADMISSION_FAILED);
    assert_eq!(
        campaign.data(campaign.plane.source_archive.address).await,
        one_record,
        "a refused seal leaves the page byte-identical"
    );
}

#[tokio::test]
async fn a_v1_page_can_never_satisfy_a_v2_spec() {
    let mut campaign = Campaign::start(registered_spec()).await;
    assert_eq!(campaign.send(campaign.init_spec()).await.0, Ok(()));
    assert_eq!(campaign.send(campaign.init_archive()).await.0, Ok(()));
    for index in 0..SPAN {
        assert_eq!(campaign.ingest(index).await.0, Ok(()));
    }
    assert_eq!(campaign.send(campaign.seal(SPAN)).await.0, Ok(()));

    let sealed = campaign.data(campaign.plane.source_archive.address).await;

    /* The v2 page's geometry is the V1 page's byte for byte, which is exactly
     * why the account tag has to carry the distinction. Flip that one byte to
     * the V1 archive tag and present the otherwise perfect page: the v2 page
     * verifier refuses it before any binding is compared. Its commitment is
     * also computed under a different domain, so the same bytes could not
     * satisfy the V1 verifier either. */
    let mut as_v1 = sealed.clone();
    assert_eq!(as_v1[0], 0x74, "the sealed page carries the v2 archive tag");
    as_v1[0] = 0x71;
    campaign.context.set_account(
        &campaign.plane.source_archive.address,
        &genesis_account(as_v1, PROGRAM_ID, false).into(),
    );
    let (result, _) = campaign.send(campaign.resolve()).await;
    assert_eq!(
        custom(&result),
        RESOLUTION_EVIDENCE_UNAVAILABLE,
        "a page wearing the other generation's tag is not evidence"
    );

    /* Cross-generation commitment confusion: restore the tag but leave the
     * page commitment recomputed under the *other* domain — here simulated by
     * corrupting the stored commitment, which is what a page carried over from
     * a V1 seal would present. */
    let mut wrong_commitment = sealed.clone();
    wrong_commitment[ARCHIVE_COMMITMENT_OFFSET] ^= 0xff;
    campaign.context.set_account(
        &campaign.plane.source_archive.address,
        &genesis_account(wrong_commitment, PROGRAM_ID, false).into(),
    );
    let (result, _) = campaign.send(campaign.resolve()).await;
    assert_eq!(custom(&result), RESOLUTION_EVIDENCE_UNAVAILABLE);

    // Restored, the honest page still resolves.
    campaign.context.set_account(
        &campaign.plane.source_archive.address,
        &genesis_account(sealed, PROGRAM_ID, false).into(),
    );
    assert_eq!(campaign.send(campaign.resolve()).await.0, Ok(()));
}

#[tokio::test]
async fn a_release_this_elf_does_not_carry_cannot_found_its_source_at_all() {
    /* The registry gate cannot be reached by presenting an unregistered body
     * to a *registered* market: the v2 feed identity is a digest over the whole
     * spec body, adapter and parser release included, so changing the release
     * changes the feed and the Terms binding refuses first. The reachable shape
     * is the one the promotion plan actually cares about — a market whose Terms
     * were frozen around a release this ELF does not carry — and it is what
     * this campaign builds. */
    let mut campaign = Campaign::start(unregistered_spec()).await;
    let (result, _) = campaign.send(campaign.init_spec()).await;
    assert_eq!(
        custom(&result),
        SOURCE_RELEASE_UNAVAILABLE,
        "0x79 stands for a market whose release is not compiled in"
    );
    assert!(
        campaign
            .maybe_account(campaign.plane.source_spec.address)
            .await
            .is_none(),
        "a refused founding creates nothing"
    );
    assert!(
        campaign
            .maybe_account(campaign.plane.feed.address)
            .await
            .is_none(),
        "and founds no feed head either"
    );
}

#[tokio::test]
async fn custody_opens_against_the_spec_this_family_just_founded() {
    /* `r2_pull_endow.rs` showed the default ELF taking custody against a v2
     * spec *installed at genesis*, because nothing could create one. This is
     * the same boundary against a spec the chain founded a transaction
     * earlier: 0x79 before, accepted after, with the same market and the same
     * Terms. Only the spec account's existence changes. */
    let mut campaign = Campaign::start(registered_spec()).await;

    let (result, _) = campaign.send(campaign.endow(DEPOSIT)).await;
    assert_eq!(
        custom(&result),
        WRONG_PROGRAM_OWNER,
        "an absent SourceSpec is a System-owned hole, and custody refuses it"
    );

    assert_eq!(campaign.send(campaign.init_spec()).await.0, Ok(()));

    let hoard_token_before = campaign
        .token_amount(campaign.plane.hoard_token.address)
        .await;
    let (result, endow_cu) = campaign.send(campaign.endow(DEPOSIT)).await;
    assert_eq!(result, Ok(()), "Endow must be accepted after the founding");
    assert_eq!(
        campaign.token_amount(ENDOW_OWNER_TOKEN).await,
        0,
        "the depositor's whole balance moved"
    );
    assert_eq!(
        campaign
            .token_amount(campaign.plane.hoard_token.address)
            .await,
        hoard_token_before + DEPOSIT,
        "and landed in the Hoard's real Token-2022 account"
    );
    let (position, _) = campaign.plane.owner_plane(campaign.endow_owner.pubkey());
    let position = PositionAccount::decode(&campaign.data(position.address).await)
        .expect("Endow created the depositor's position");
    assert_eq!(position.cash_atoms, DEPOSIT);
    println!("r2 v2 wire CU: endow={endow_cu}");
}

/// Real deployed provider/ABI/crypto execution over a synthetic local
/// observation. Router verification first persists a Verified VAA; only the
/// subsequent receiver `PostUpdate` and Clutch append are atomic and adjacent.
/// This is intentionally not devnet price evidence.
#[cfg(feature = "non-production-real-pyth-lab")]
#[tokio::test]
async fn real_pyth_router_verifies_then_post_update_and_clutch_append_are_atomic() {
    use clutch_sbf::pyth_receiver::{parse_full_price_update_v2, PriceUpdateAccountViewV1};

    let mut campaign = Campaign::start_real_pyth_one_bucket_categorical(real_pyth_spec(
        real_pyth_lab::PROVIDER_FEED_ID,
    ))
    .await;
    let publish_boundary = u64::try_from(REAL_PYTH_PUBLISH_TIME).unwrap() / 60;
    assert_eq!(campaign.start_bucket + 1, publish_boundary);
    assert_eq!(campaign.end_bucket_exclusive, publish_boundary);
    campaign.initialize_real_pyth().await;
    assert_eq!(campaign.send(campaign.init_spec()).await.0, Ok(()));
    assert_eq!(campaign.send(campaign.init_archive()).await.0, Ok(()));
    let genesis_page = campaign.data(campaign.plane.source_archive.address).await;
    let update = campaign.update.pubkey();
    assert!(campaign.maybe_account(update).await.is_none());

    // No adjacent receiver call: the archive refuses before source parsing.
    let (result, _) = campaign.send(campaign.append(0, update)).await;
    assert_eq!(custom(&result), SOURCE_ADMISSION_FAILED);
    assert_eq!(
        campaign.data(campaign.plane.source_archive.address).await,
        genesis_page
    );

    // The real receiver successfully creates and writes the update, then the
    // Clutch half sees a mismatched Config. The failed transaction must roll
    // back both the provider write and the archive mutation.
    let wrong_config = Address::new_from_array([0xcf; 32]);
    let receiver = Address::new_from_array(real_pyth_lab::RECEIVER_PROGRAM);
    let treasury = Address::find_program_address(&[b"treasury", &[0]], &receiver).0;
    let payer = campaign.context.payer.pubkey();
    let payer_before = campaign.maybe_account(payer).await;
    let treasury_before = campaign.maybe_account(treasury).await;
    let (result, _) = campaign
        .send_many(&[
            campaign.real_pyth_post(update),
            campaign.append_with_config(0, update, wrong_config),
        ])
        .await;
    assert_eq!(custom(&result), SOURCE_ADMISSION_FAILED);
    assert!(
        campaign.maybe_account(update).await.is_none(),
        "Clutch refusal must roll back the receiver-created account"
    );
    assert_eq!(
        campaign.data(campaign.plane.source_archive.address).await,
        genesis_page,
        "Clutch refusal must leave the archive byte-identical"
    );
    assert_eq!(campaign.maybe_account(treasury).await, treasury_before);
    let payer_after = campaign.maybe_account(payer).await.unwrap();
    let payer_before = payer_before.unwrap();
    assert_eq!(payer_after.data, payer_before.data);
    assert_eq!(payer_after.owner, payer_before.owner);
    assert_eq!(
        payer_before.lamports - payer_after.lamports,
        10_000,
        "only the failed two-signature transaction fee survives rollback"
    );

    // The exact real PostUpdate and Clutch append are adjacent in this one
    // transaction. No mock writer or preinstalled update account participates.
    let (result, joined_cu) = campaign
        .send_many(&[campaign.real_pyth_post(update), campaign.append(0, update)])
        .await;
    assert_eq!(result, Ok(()));
    let update_account = campaign.maybe_account(update).await.unwrap();
    let parsed = parse_full_price_update_v2(
        PriceUpdateAccountViewV1::new(
            update.to_bytes(),
            update_account.owner.to_bytes(),
            update_account.executable,
            &update_account.data,
        ),
        real_pyth_lab::RECEIVER_PROGRAM,
        real_pyth_lab::PROVIDER_FEED_ID,
    )
    .expect("the real receiver wrote a canonical Full update");
    assert_eq!(parsed.price, 100_000_000);
    assert_eq!(parsed.confidence, 6_357);
    assert_eq!(parsed.exponent, -8);
    assert_eq!(parsed.publish_time, REAL_PYTH_PUBLISH_TIME);
    let captured_update = real_pyth_fixture("price-update.account");
    assert_eq!(&update_account.data[..8], &captured_update[..8]);
    assert_eq!(
        &update_account.data[40..125],
        &captured_update[40..125],
        "verification level and every VAA-owned price field match the captured receiver result"
    );
    assert_eq!(update_account.data[133], captured_update[133]);
    // Bytes 8..40 (write authority) and 125..133 (receiver-write slot) are
    // transaction-local facts, so exact equality there would be false evidence.

    let page = campaign.data(campaign.plane.source_archive.address).await;
    assert_ne!(page, genesis_page);
    assert_eq!(page[3], 1, "exactly one authenticated record was appended");
    let record = &page[512..576];
    let u64_at = |at: usize| u64::from_le_bytes(record[at..at + 8].try_into().unwrap());
    let u128_at = |at: usize| u128::from_le_bytes(record[at..at + 16].try_into().unwrap());
    assert_eq!(u64_at(0), campaign.start_bucket);
    assert_eq!(u128_at(8), 99_980_929);
    assert_eq!(u128_at(24), 100_019_071);
    assert_eq!(u64_at(40), REAL_PYTH_PUBLISH_TIME as u64);
    assert_eq!(u64_at(48), parsed.posted_slot);
    assert_eq!(u64_at(56), REAL_PYTH_PUBLISH_TIME as u64);

    // This is a complete one-bucket market, not merely an admitted source
    // record. The exact nonzero-confidence interval lies wholly between the
    // 99m and 101m categorical knots, so no caller discretion selects cell 1.
    let lower = u128_at(8);
    let upper = u128_at(24);
    assert!(99_000_000 < lower);
    assert!(lower <= upper);
    assert!(upper < 101_000_000);
    let (result, seal_cu) = campaign.send(campaign.seal(1)).await;
    assert_eq!(result, Ok(()), "the complete one-record page must seal");
    let feed = FeedAccount::decode(&campaign.data(campaign.plane.feed.address).await)
        .expect("the one-bucket real-Pyth feed decodes after seal");
    assert_eq!(feed.cursor, campaign.end_bucket_exclusive);
    assert_eq!(feed.archive_pages, 1);

    let (result, resolve_cu) = campaign.send(campaign.resolve_with_payout(1)).await;
    assert_eq!(
        result,
        Ok(()),
        "the authenticated interval wholly selects categorical cell 1"
    );
    let resolution =
        ResolutionAccount::decode(&campaign.data(campaign.plane.resolution.address).await)
            .expect("the one-bucket real-Pyth categorical resolution decodes");
    assert_eq!(resolution.payout_index, 1);
    assert_eq!(
        resolution.sealed_end_bucket_exclusive,
        campaign.end_bucket_exclusive
    );
    assert_eq!(resolution.feed_cursor, campaign.end_bucket_exclusive);
    let market =
        MarketAccount::decode(&campaign.data(campaign.plane.market.address).await).expect("market");
    assert_eq!(market.lifecycle, 1, "the one-bucket market is resolved");

    // A spec pinning another feed still selects the same reviewed release, but
    // the real proof writes 0x2a... and the account-level feed join refuses.
    let mut wrong_feed = Campaign::start(real_pyth_spec([0x2b; 32])).await;
    wrong_feed.initialize_real_pyth().await;
    assert_eq!(wrong_feed.send(wrong_feed.init_spec()).await.0, Ok(()));
    assert_eq!(wrong_feed.send(wrong_feed.init_archive()).await.0, Ok(()));
    let wrong_page = wrong_feed
        .data(wrong_feed.plane.source_archive.address)
        .await;
    let wrong_update = wrong_feed.update.pubkey();
    let (result, _) = wrong_feed
        .send_many(&[
            wrong_feed.real_pyth_post(wrong_update),
            wrong_feed.append(0, wrong_update),
        ])
        .await;
    assert_eq!(custom(&result), SOURCE_ADMISSION_FAILED);
    assert!(wrong_feed.maybe_account(wrong_update).await.is_none());
    assert_eq!(
        wrong_feed
            .data(wrong_feed.plane.source_archive.address)
            .await,
        wrong_page,
        "wrong-feed refusal rolls back provider and Clutch writes"
    );

    println!(
        "local-real Pyth CU: joined_post_update_plus_clutch_append={joined_cu} \
         seal_one_bucket={seal_cu} resolve_categorical={resolve_cu}"
    );
}
