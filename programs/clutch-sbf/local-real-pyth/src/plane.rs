//! Test-only one-bucket categorical market/source planes and instructions.

use crate::provider;
use clutch_kernel::{PayoutSet, PayoutVector};
use clutch_sbf::{
    instructions::observe_resolve,
    seeds, source_archive_v2,
    source_identity::real_pyth_lab,
    source_v2::{
        crossing::SELECTION_CROSSING_V1,
        spec::{
            SourceSpecFieldsV2, SourceSpecV2, GRID_ORIGIN_UNIX_SECONDS_V1,
            ORIENTATION_QUOTE_PER_BASE,
        },
    },
};
use clutch_solana_layout::{
    account_len, canonical_outcome_id, FeedAccount, Hash32, HoardAccount, Intent, MarketAccount,
    PayoutVectorBytes, PositionAccount, ResolutionAccount, SupplyLedgerAccount, TermsAccount,
    MAX_KNOTS, MAX_OUTCOMES, MAX_PAYOUTS, PAYOUT_INDEX_UNRESOLVED, PAYOUT_MAP_UNUSED,
};
use clutch_solana_reference::{KernelAccount, STAT_TERMINAL_01};
use clutch_svm_fixture::{
    build_plane, fixture_terms, layout_request, GenesisAccount, Mode, Pda, Plane, MARKET_NONCE,
    PROGRAM_ID, RENT_SYSVAR, SYSTEM_PROGRAM, TOKEN_2022,
};
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};

const OUTCOMES: u8 = 4;
const DENOMINATOR: u64 = 64;
const SETS: u64 = 64;
pub const COLLATERAL_MINT: Address = Address::new_from_array([0x6c; 32]);
pub const WRONG_MARKET_NONCE: u64 = MARKET_NONCE + 1;

pub struct LabPlane {
    pub plane: Plane,
    pub spec: SourceSpecV2,
    pub start_bucket: u64,
    pub end_bucket_exclusive: u64,
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
    assert_eq!(encoder(&mut out).expect("laboratory fixture encodes"), len);
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

pub fn real_spec(feed_id: [u8; 32]) -> Result<SourceSpecV2, Box<dyn std::error::Error>> {
    let config = provider::fixture("receiver-config.account")?;
    SourceSpecV2::new(SourceSpecFieldsV2 {
        source_adapter_id: real_pyth_lab::RELEASE.source_adapter_id,
        source_adapter_version: real_pyth_lab::RELEASE.source_adapter_version,
        parser_id: real_pyth_lab::RELEASE.parser_id,
        parser_version: real_pyth_lab::RELEASE.parser_version,
        receiver_program: real_pyth_lab::RECEIVER_PROGRAM,
        receiver_programdata: real_pyth_lab::RECEIVER_PROGRAMDATA,
        receiver_config: real_pyth_lab::RECEIVER_CONFIG,
        config_digest: clutch_sbf::pyth_receiver::config_byte_digest(&config),
        provider_feed_id: feed_id,
        programdata_deployment_slot: real_pyth_lab::RECEIVER_DEPLOYMENT_SLOT,
        base_asset_id: real_pyth_lab::BASE_ASSET_ID,
        quote_asset_id: real_pyth_lab::QUOTE_ASSET_ID,
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
    })
    .map_err(|error| format!("local-real SourceSpec is invalid: {error:?}").into())
}

fn window_identity(terms: &TermsAccount, feed: Hash32) -> Hash32 {
    let identity = clutch_sbf::source_archive::FeedIdentity::new(
        terms.source_adapter_id.bytes(),
        feed.bytes(),
        terms.source_version,
        terms.evaluator_version,
    )
    .expect("laboratory feed identity");
    let grid = clutch_sbf::source_archive::Grid::new(
        terms.grid_family_id,
        terms.grid_version,
        terms.bucket_seconds,
    )
    .expect("laboratory grid");
    let window = clutch_sbf::source_archive::WindowDomain::new(
        identity,
        grid,
        terms.expected_start_bucket,
        terms.expected_end_bucket_exclusive,
        terms.expected_start_bucket + terms.maturity_horizon_buckets,
        terms.repair_generation,
        clutch_sbf::source_archive::CoveragePolicy::COMPLETE_REQUIRED,
    )
    .expect("laboratory window");
    source_archive_v2::canonical_window_id(window)
}

pub fn build(
    actor: Address,
    spec: SourceSpecV2,
    start_bucket: u64,
    end_bucket_exclusive: u64,
    market_nonce: u64,
) -> LabPlane {
    assert_eq!(end_bucket_exclusive, start_bucket + 1);
    let feed_id = Hash32::from_bytes(spec.feed_id());
    let mut plane = build_plane(actor, COLLATERAL_MINT, market_nonce, Mode::Funded);
    let old_terms_address = plane.terms.address;
    let market_address = plane.market.address;
    let position_address = plane.position.address;
    let kernel_address = plane.kernel.address;
    let supply_address = plane.supply.address;
    let hoard_address = plane.hoard.address;
    let resolution_address = plane.resolution.address;
    let (payout_bytes, payout_set) = one_hot_payouts();

    let mut terms = fixture_terms(plane.realm_id, plane.profile_id, feed_id);
    terms.source_adapter_id = Hash32::from_bytes(real_pyth_lab::RELEASE.source_adapter_id);
    terms.source_version = real_pyth_lab::RELEASE.source_adapter_version;
    terms.outcome_count = OUTCOMES;
    terms.payout_count = OUTCOMES;
    terms.payouts = payout_bytes;
    terms.statistic_id = STAT_TERMINAL_01;
    terms.basis_degree = 0;
    terms.knot_count = OUTCOMES - 1;
    terms.uniform_log2_spacing = clutch_solana_layout::UNIFORM_SPACING_NONE;
    terms.knots = [0; MAX_KNOTS];
    terms.knots[..3].copy_from_slice(&[99_000_000, 101_000_000, 102_000_000]);
    terms.payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
    for payout in 0..OUTCOMES {
        terms.payout_map[usize::from(payout)] = payout;
    }
    terms.expected_start_bucket = start_bucket;
    terms.expected_end_bucket_exclusive = end_bucket_exclusive;
    terms.maturity_horizon_buckets = 2;
    terms.terms = Hash32::ZERO;
    terms.terms = terms
        .recomputed_terms_digest()
        .expect("laboratory Terms body digests");
    let terms_id = terms.terms;
    let terms_pda = pda(&[
        seeds::SEED_TERMS,
        &plane.realm_id.bytes(),
        &terms_id.bytes(),
    ]);
    terms.stored_bump = terms_pda.bump;
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
    let mut market = MarketAccount::decode(&account_mut(&mut plane, market_address).data)
        .expect("market decodes");
    market.terms = terms_id;
    market.feed = feed_id;
    market.outcome_count = OUTCOMES;
    market.outcomes = outcomes;
    account_mut(&mut plane, market_address).data =
        encode(account_len::MARKET, |out| market.encode(out));

    let mut internal = [0_u64; MAX_OUTCOMES];
    internal[..usize::from(OUTCOMES)].fill(SETS);
    let mut position = PositionAccount::decode(&account_mut(&mut plane, position_address).data)
        .expect("position decodes");
    position.internal = internal;
    account_mut(&mut plane, position_address).data =
        encode(account_len::POSITION, |out| position.encode(out));

    let kernel = KernelAccount {
        market: market_id,
        phase: 0,
        basis_mode: clutch_kernel::BasisMode::FinitePreset,
        resolved_payout: 0,
        payouts: payout_set,
        total_supply: internal,
    };
    account_mut(&mut plane, kernel_address).data =
        encode(clutch_solana_reference::KERNEL_ACCOUNT_LEN, |out| {
            kernel.encode(out)
        });
    let mut supply = SupplyLedgerAccount::decode(&account_mut(&mut plane, supply_address).data)
        .expect("supply decodes");
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

    LabPlane {
        plane,
        spec,
        start_bucket,
        end_bucket_exclusive,
    }
}

pub fn init_spec(actor: Address, lab: &LabPlane) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::InitSourceSpecV2 {
                terms: lab.plane.terms_id,
                spec_body: lab.spec.encode_canonical(),
            },
        ),
        vec![
            AccountMeta::new(actor, true),
            AccountMeta::new(lab.plane.source_spec.address, false),
            AccountMeta::new(lab.plane.feed.address, false),
            AccountMeta::new_readonly(lab.plane.terms.address, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ],
    )
}

pub fn init_archive(actor: Address, lab: &LabPlane) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::InitSourceArchiveV2 {
                terms: lab.plane.terms_id,
            },
        ),
        vec![
            AccountMeta::new(actor, true),
            AccountMeta::new_readonly(lab.plane.source_spec.address, false),
            AccountMeta::new_readonly(lab.plane.feed.address, false),
            AccountMeta::new_readonly(lab.plane.terms.address, false),
            AccountMeta::new(lab.plane.source_archive.address, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ],
    )
}

pub fn append(lab: &LabPlane, update: Address, config: Address) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::AppendSourceArchiveV2 {
                terms: lab.plane.terms_id,
            },
        ),
        vec![
            AccountMeta::new_readonly(lab.plane.source_spec.address, false),
            AccountMeta::new_readonly(lab.plane.feed.address, false),
            AccountMeta::new_readonly(lab.plane.terms.address, false),
            AccountMeta::new(lab.plane.source_archive.address, false),
            AccountMeta::new_readonly(
                Address::new_from_array(lab.spec.fields().receiver_program),
                false,
            ),
            AccountMeta::new_readonly(
                Address::new_from_array(lab.spec.fields().receiver_programdata),
                false,
            ),
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(update, false),
            AccountMeta::new_readonly(
                Address::new_from_array(clutch_sbf::instructions_sysvar::INSTRUCTIONS_SYSVAR_ID),
                false,
            ),
            AccountMeta::new_readonly(
                Address::new_from_array(clutch_sbf::source_identity::CLOCK_SYSVAR_ID),
                false,
            ),
        ],
    )
}

pub fn seal(lab: &LabPlane) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            1,
            Intent::SealSourceArchiveV2 {
                terms: lab.plane.terms_id,
            },
        ),
        vec![
            AccountMeta::new_readonly(lab.plane.source_spec.address, false),
            AccountMeta::new(lab.plane.feed.address, false),
            AccountMeta::new_readonly(lab.plane.terms.address, false),
            AccountMeta::new(lab.plane.source_archive.address, false),
        ],
    )
}

pub fn resolve(actor: Address, lab: &LabPlane, payout_index: u8) -> Instruction {
    let mut data = vec![0xd1, 1];
    data.extend_from_slice(&0_u64.to_le_bytes());
    data.push(1);
    data.push(payout_index);
    let mut metas = vec![
        AccountMeta::new_readonly(actor, true),
        AccountMeta::new(lab.plane.market.address, false),
        AccountMeta::new_readonly(lab.plane.hoard.address, false),
        AccountMeta::new(lab.plane.kernel.address, false),
        AccountMeta::new(lab.plane.supply.address, false),
        AccountMeta::new_readonly(lab.plane.terms.address, false),
        AccountMeta::new(lab.plane.resolution.address, false),
        AccountMeta::new_readonly(lab.plane.feed.address, false),
        AccountMeta::new_readonly(lab.plane.source_spec.address, false),
        AccountMeta::new_readonly(lab.plane.source_archive.address, false),
    ];
    metas.extend(
        lab.plane
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

pub fn decode_feed(data: &[u8]) -> Result<FeedAccount, Box<dyn std::error::Error>> {
    FeedAccount::decode(data).map_err(|error| format!("feed does not decode: {error:?}").into())
}

pub fn decode_resolution(data: &[u8]) -> Result<ResolutionAccount, Box<dyn std::error::Error>> {
    ResolutionAccount::decode(data)
        .map_err(|error| format!("resolution does not decode: {error:?}").into())
}

pub fn decode_market(data: &[u8]) -> Result<MarketAccount, Box<dyn std::error::Error>> {
    MarketAccount::decode(data).map_err(|error| format!("market does not decode: {error:?}").into())
}

pub fn token_program() -> Address {
    TOKEN_2022
}
