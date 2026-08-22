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
//! `clutch_lab_receiver.so` is that program: a no-op with no dependencies,
//! installed at `source_identity::fixture::RECEIVER_PROGRAM` under a fabricated
//! Upgradeable Loader program/ProgramData pair. It is **not** a model of Pyth's
//! receiver and implements no part of `post_update`. It does not have to be:
//! `source_v2::auth` never trusts the receiver's behaviour. It authenticates
//! the receiver *deployment* (pinned key, loader ownership, the ProgramData
//! link and its deployment slot, the governance `Config` digest) and separately
//! authenticates *adjacency*, and it reads the price out of the update
//! account's own 134 bytes. A no-op stand-in is therefore the honest fixture:
//! nothing it does can accidentally supply evidence.
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
        PositionAccount, SupplyLedgerAccount, TermsAccount, MAX_KNOTS, MAX_OUTCOMES, MAX_PAYOUTS,
        PAYOUT_INDEX_UNRESOLVED, PAYOUT_MAP_UNUSED,
    },
    clutch_solana_reference::KernelAccount,
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

const ACTOR_TOKEN: Address = Address::new_from_array([0x8e; 32]);
/// A second owner, with no position, who takes custody after the founding.
const ENDOW_OWNER_TOKEN: Address = Address::new_from_array([0x8f; 32]);
/// Collateral atoms that owner deposits.
const DEPOSIT: u64 = 500;
const UPDATE_ACCOUNT: Address = Address::new_from_array([0xc1; 32]);
const WRITE_AUTHORITY: Address = Address::new_from_array([0xc2; 32]);
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

/// Everything the campaign needs to address one v2-bound occupation market.
struct PullPlane {
    plane: Plane,
    spec: SourceSpecV2,
    start_bucket: u64,
    end_bucket_exclusive: u64,
}

/// Build a degree-one occupation market bound to one v2 pull spec and one
/// window, with the source spec, feed head and archive **absent**: those are
/// exactly what the new intents create.
fn pull_occupation_plane(actor: Address, spec: SourceSpecV2, start_bucket: u64) -> PullPlane {
    let end_bucket_exclusive = start_bucket + SPAN;
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
    terms.statistic_id = STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06;
    terms.basis_degree = BASIS_DEGREE;
    terms.knot_count = OUTCOMES + 1 - BASIS_DEGREE;
    terms.uniform_log2_spacing = 3;
    terms.payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
    terms.knots = [0; MAX_KNOTS];
    for (index, knot) in terms
        .knots
        .iter_mut()
        .take(usize::from(terms.knot_count))
        .enumerate()
    {
        *knot = (index as u128) * 8;
    }
    terms.expected_start_bucket = start_bucket;
    terms.expected_end_bucket_exclusive = end_bucket_exclusive;
    /* `read_frozen_terms` requires the maturity bucket to be exactly one past
     * the window end, so the horizon is the span plus one. */
    terms.maturity_horizon_buckets = SPAN + 1;
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
        basis_mode: clutch_kernel::BasisMode::DerivedBasis,
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

    let unresolved = OccupationResolutionAccount::unresolved(
        market_id,
        terms_id,
        feed_id,
        plane.resolution.bump,
    );
    account_mut(&mut plane, resolution_address).data =
        encode(OCCUPATION_RESOLUTION_LEN, |out| unresolved.encode(out));

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
    plane: Plane,
    spec: SourceSpecV2,
    start_bucket: u64,
    end_bucket_exclusive: u64,
}

impl Campaign {
    async fn start(spec: SourceSpecV2) -> Self {
        let actor = actor_keypair();
        let mut test = ProgramTest::default();
        test.prefer_bpf(true);
        test.add_program("clutch_sbf", PROGRAM_ID, None);

        /* The fabricated receiver deployment goes in at genesis, not through
         * `set_account`, so the program account's last modification slot is
         * zero and the loader sees a settled deployment. */
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
        let mut funded = genesis_account(Vec::new(), SYSTEM_PROGRAM, false);
        funded.lamports = 10_000_000_000;
        test.add_account(actor.pubkey(), funded.clone());
        test.add_account(endow_owner_keypair().pubkey(), funded);

        let mut context = test.start_with_context().await;
        /* One small warp: a program is invisible until one slot past its
         * recorded deployment, and the campaign's freshness bounds want a slot
         * comfortably above zero. */
        context.warp_to_slot(WARP_SLOT).expect("warp");

        let (slot, unix) = clock(&mut context).await;
        /* Place the whole window in the settled past: the last bucket's closing
         * boundary is at least two minutes behind the Clock, so every append's
         * boundary-plus-grace maturity check is satisfied, while the first
         * boundary is well inside the spec's 600-second staleness bound. */
        let end_bucket_exclusive = (unix as u64 - 120) / 60;
        let start_bucket = end_bucket_exclusive - SPAN;
        assert!(
            start_bucket > 0,
            "the bank clock must be past the epoch for a 60-second grid"
        );

        let built = pull_occupation_plane(actor.pubkey(), spec, start_bucket);
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

    /// The preceding instruction: a real call into the laboratory receiver,
    /// laid out at the fixture ABI's meta positions (config 2, update 4, write
    /// authority 6).
    fn post(&self, update: Address) -> Instruction {
        Instruction::new_with_bytes(
            Address::new_from_array(fixture::RECEIVER_PROGRAM),
            &[0xaa, 0xbb, 0xcc],
            vec![
                AccountMeta::new_readonly(Address::new_from_array([0x01; 32]), false),
                AccountMeta::new_readonly(Address::new_from_array([0x02; 32]), false),
                AccountMeta::new_readonly(Address::new_from_array(fixture::RECEIVER_CONFIG), false),
                AccountMeta::new_readonly(Address::new_from_array([0x04; 32]), false),
                AccountMeta::new_readonly(update, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(WRITE_AUTHORITY, false),
            ],
        )
    }

    fn append(&self, sequence: u64, update: Address) -> Instruction {
        let data = layout_request(
            sequence,
            Intent::AppendSourceArchiveV2 {
                terms: self.plane.terms_id,
            },
        );
        self.append_with_archive(data, update, self.plane.source_archive.address)
    }

    fn append_with_archive(&self, data: Vec<u8>, update: Address, archive: Address) -> Instruction {
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &data,
            vec![
                AccountMeta::new_readonly(self.plane.source_spec.address, false),
                AccountMeta::new_readonly(self.plane.feed.address, false),
                AccountMeta::new_readonly(self.plane.terms.address, false),
                AccountMeta::new(archive, false),
                AccountMeta::new_readonly(
                    Address::new_from_array(fixture::RECEIVER_PROGRAM),
                    false,
                ),
                AccountMeta::new_readonly(
                    Address::new_from_array(fixture::RECEIVER_PROGRAMDATA),
                    false,
                ),
                AccountMeta::new_readonly(Address::new_from_array(fixture::RECEIVER_CONFIG), false),
                AccountMeta::new_readonly(update, false),
                AccountMeta::new_readonly(INSTRUCTIONS_SYSVAR, false),
                AccountMeta::new_readonly(CLOCK_SYSVAR, false),
            ],
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
        self.resolve_with_archive(self.plane.source_archive.address)
    }

    fn resolve_with_archive(&self, archive: Address) -> Instruction {
        let mut data = vec![0xd1, 1];
        data.extend_from_slice(&0_u64.to_le_bytes());
        data.push(1);
        data.push(PAYOUT_INDEX_UNRESOLVED);
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
            observe_resolve::OCCUPATION_RESOLVE_ACCOUNT_PREFIX + usize::from(OUTCOMES)
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

    /// Install the ephemeral update account for one boundary, then drive the
    /// receiver post and the append together.
    async fn ingest(&mut self, index: u64) -> (Result<(), TransactionError>, u64) {
        let bucket = self.start_bucket + index;
        self.install_update(UPDATE_ACCOUNT, bucket, self.spec.fields().provider_feed_id)
            .await;
        self.ingest_now(index, UPDATE_ACCOUNT).await
    }

    /// Drive the post/append pair against whatever update account is already
    /// installed, so the hostile battery can install a bad one first.
    async fn ingest_now(
        &mut self,
        index: u64,
        update: Address,
    ) -> (Result<(), TransactionError>, u64) {
        let post = self.post(update);
        let append = self.append(index, update);
        self.send_many(&[post, append]).await
    }

    async fn install_update(&mut self, at: Address, bucket: u64, feed_id: [u8; 32]) {
        let (slot, _) = clock(&mut self.context).await;
        let boundary = (bucket + 1) as i64 * 60;
        let update = PriceUpdateFixture {
            write_authority: WRITE_AUTHORITY.to_bytes(),
            verification_level: 1,
            feed_id,
            price: PRICE_ATOMS,
            /* Zero confidence: the occupation fold admits only a point
             * observation and refuses to invent a midpoint. */
            confidence: 0,
            exponent: -8,
            publish_time: boundary,
            prev_publish_time: boundary - 1,
            ema_price: PRICE_ATOMS,
            ema_confidence: 0,
            posted_slot: slot,
            trailing_pad: 0,
        };
        let data = price_update_body(update);
        assert_eq!(data.len(), PRICE_UPDATE_V2_ACCOUNT_LEN);
        self.context.set_account(
            &at,
            &genesis_account(
                data,
                Address::new_from_array(fixture::RECEIVER_PROGRAM),
                false,
            )
            .into(),
        );
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
            } else {
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

/* ------------------------------------------------------------------------ */
/* The hostile battery                                                       */
/* ------------------------------------------------------------------------ */

/// Refusal codes this battery pins, by name.
const SOURCE_RELEASE_UNAVAILABLE: u32 = 0x0079;
const SOURCE_ADMISSION_FAILED: u32 = 0x007a;
const ALREADY_INITIALIZED: u32 = 0x0040;
const REPLAY: u32 = 0x000d;
const WRONG_PROGRAM_OWNER: u32 = 0x0004;
const RESOLUTION_EVIDENCE_UNAVAILABLE: u32 = 0x0010;

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
    campaign
        .install_update(UPDATE_ACCOUNT, bucket, fixture::PROVIDER_FEED_ID)
        .await;

    // No post at all: the preceding instruction is the compute-budget one.
    let (result, _) = campaign.send(campaign.append(0, UPDATE_ACCOUNT)).await;
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
        .send_many(&[wrong_program, campaign.append(0, UPDATE_ACCOUNT)])
        .await;
    assert_eq!(custom(&result), SOURCE_ADMISSION_FAILED);

    // A post that names a *different* update account than the one presented.
    let decoy = Address::new_from_array([0xd0; 32]);
    campaign
        .install_update(decoy, bucket, fixture::PROVIDER_FEED_ID)
        .await;
    let (result, _) = campaign
        .send_many(&[campaign.post(decoy), campaign.append(0, UPDATE_ACCOUNT)])
        .await;
    assert_eq!(custom(&result), SOURCE_ADMISSION_FAILED);

    // A post naming the presented account, but the update carries a different
    // provider feed than the immutable spec pins.
    campaign
        .install_update(UPDATE_ACCOUNT, bucket, [0x5f; 32])
        .await;
    let (result, _) = campaign.ingest_now(0, UPDATE_ACCOUNT).await;
    assert_eq!(custom(&result), SOURCE_ADMISSION_FAILED);

    assert_eq!(
        campaign.data(campaign.plane.source_archive.address).await,
        genesis_page,
        "every refused append leaves the page byte-identical"
    );

    // The honest append lands, and does not land twice.
    let (result, _) = campaign.ingest(0).await;
    assert_eq!(result, Ok(()));
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
