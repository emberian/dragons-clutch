//! Test-only categorical market/source planes and instructions.

use crate::provider;
use clutch_batch::relation_v1::{
    canonical_candidate, canonical_pairing, BookV1, LegRefV1, RelationDomainV1,
};
use clutch_batch_policy_identity::{
    batch_policy_digest, canonical_batch_policy_bytes,
    general_clearing_v1::GENERAL_CLEARING_POLICY_V1,
};
use clutch_kernel::{PayoutSet, PayoutVector};
use clutch_sbf::source_archive::WindowDomain;
use clutch_sbf::{
    instructions::{cash_exit, genesis, market_init, observe_resolve, split as seam},
    seeds, source_archive_v2,
    source_identity::real_pyth_lab,
    source_v2::spec::SourceSpecV2,
};
use clutch_solana_layout::artifact::{ArtifactBinding, ArtifactKind, ARTIFACT_CHUNK_BYTES};
use clutch_solana_layout::clearing::{LegRef, PairingSlice};
use clutch_solana_layout::projection::{project_slot, OwnerInterner};
use clutch_solana_layout::reservation::canonical_reservation_id;
use clutch_solana_layout::{
    account_len, canonical_epoch_id, canonical_order_id, canonical_outcome_id, stream,
    CandidateFeedChunk, CandidateRecord, EpochAccount, FeedAccount, Hash32, HoardAccount, Intent,
    MarketAccount, OrderRecord, OrderSlot, PayoutVectorBytes, PositionAccount, PriceGridAccount,
    ResolutionAccount, SupplyLedgerAccount, TermsAccount, CANDIDATE_STATUS_SUBMITTED,
    FEED_FILLS_PER_CHUNK, FEED_SLICES_PER_CHUNK, MAX_GRID_TICKS, MAX_KNOTS, MAX_OUTCOMES,
    MAX_PAYOUTS, PAYOUT_INDEX_UNRESOLVED, PAYOUT_MAP_UNUSED,
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
pub const USER_COLLATERAL_ATOMS: u64 = SETS;
pub const JOINED_COLLATERAL_SUPPLY: u64 = USER_COLLATERAL_ATOMS * 2;
pub const PRICE_SCALE: u64 = 10_000;
pub const GENERAL_EPOCH_INDEX: u64 = 1;
pub const ORDER_QUANTITY: u64 = 16;
pub const BUY_LIMIT: u64 = 7_500;
pub const SELL_LIMIT: u64 = 2_500;
pub const COLLATERAL_MINT: Address = Address::new_from_array([0x6c; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarketPrestate {
    GenesisFunded,
    SignedCreate,
}

pub struct LabPlane {
    pub plane: Plane,
    /// The canonical final PriceGrid PDA. It is deliberately absent from
    /// `plane.accounts`: joined campaigns create it through the signed typed
    /// artifact transport before opening an epoch.
    pub grid: Pda,
    pub grid_value: PriceGridAccount,
    pub grid_bytes: Vec<u8>,
    pub spec: SourceSpecV2,
    /// Exact canonical source window authenticated by the archive codec.
    pub window: WindowDomain,
    pub start_bucket: u64,
    pub end_bucket_exclusive: u64,
    pub market_prestate: MarketPrestate,
}

pub fn actor_collateral(actor: Address) -> Address {
    Address::find_program_address(
        &[b"local-real-pyth-user-collateral", actor.as_ref()],
        &PROGRAM_ID,
    )
    .0
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

pub fn real_spec() -> Result<SourceSpecV2, Box<dyn std::error::Error>> {
    let config = provider::fixture("receiver-config.account")?;
    let digest = clutch_sbf::pyth_receiver::config_byte_digest(&config);
    if digest != real_pyth_lab::CONFIG_DIGEST {
        return Err("local-real receiver Config does not match the compiled release".into());
    }
    SourceSpecV2::new(real_pyth_lab::REGISTERED_SPEC_FIELDS)
        .map_err(|error| format!("local-real SourceSpec is invalid: {error:?}").into())
}

fn source_window(terms: &TermsAccount, feed: Hash32) -> WindowDomain {
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
    clutch_sbf::source_archive::WindowDomain::new(
        identity,
        grid,
        terms.expected_start_bucket,
        terms.expected_end_bucket_exclusive,
        terms.expected_start_bucket + terms.maturity_horizon_buckets,
        terms.repair_generation,
        clutch_sbf::source_archive::CoveragePolicy::COMPLETE_REQUIRED,
    )
    .expect("laboratory window")
}

pub fn build(
    actor: Address,
    spec: SourceSpecV2,
    start_bucket: u64,
    end_bucket_exclusive: u64,
    market_nonce: u64,
    market_prestate: MarketPrestate,
) -> LabPlane {
    let bucket_count = end_bucket_exclusive
        .checked_sub(start_bucket)
        .expect("laboratory window is ordered");
    assert!(bucket_count > 0);
    assert!(bucket_count <= source_archive_v2::SOURCE_ARCHIVE_MAX_RECORDS_V2 as u64);
    let feed_id = Hash32::from_bytes(spec.feed_id());
    let mode = match market_prestate {
        MarketPrestate::GenesisFunded => Mode::Funded,
        MarketPrestate::SignedCreate => Mode::Empty,
    };
    let mut plane = build_plane(actor, COLLATERAL_MINT, market_nonce, mode);
    let old_terms_address = plane.terms.address;
    let (payout_bytes, _) = one_hot_payouts();

    let mut ticks = [0_u64; MAX_GRID_TICKS];
    ticks[..5].copy_from_slice(&[0, 2_500, 5_000, 7_500, PRICE_SCALE]);
    let mut grid_value = PriceGridAccount {
        grid: Hash32::ZERO,
        realm: plane.realm_id,
        price_scale: PRICE_SCALE,
        tick_count: 5,
        ticks,
        stored_bump: 0,
        flags: 0,
    };
    grid_value.grid = grid_value
        .recomputed_grid_id()
        .expect("laboratory PriceGrid body digests");
    let grid = pda(&[
        seeds::SEED_GRID,
        &plane.realm_id.bytes(),
        &grid_value.grid.bytes(),
    ]);
    grid_value.stored_bump = grid.bump;
    let grid_bytes = encode(account_len::PRICE_GRID, |out| grid_value.encode(out));

    let mut terms = fixture_terms(plane.realm_id, plane.profile_id, feed_id);
    terms.price_grid = grid_value.grid;
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
    // The source seam requires maturity to be the first bucket after the
    // complete observation window. A one-boundary fixture therefore uses a
    // horizon of two buckets; wider windows must grow with their exact active
    // width rather than inheriting that one-boundary constant.
    terms.maturity_horizon_buckets = bucket_count
        .checked_add(1)
        .expect("laboratory maturity horizon fits u64");
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
    grid_value
        .binds_terms(&terms)
        .expect("laboratory PriceGrid binds immutable Terms");

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
    if market_prestate == MarketPrestate::GenesisFunded {
        let market_address = plane.market.address;
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
        let position_address = plane.position.address;
        let mut position = PositionAccount::decode(&account_mut(&mut plane, position_address).data)
            .expect("position decodes");
        position.internal = internal;
        account_mut(&mut plane, position_address).data =
            encode(account_len::POSITION, |out| position.encode(out));

        let (_, payout_set) = one_hot_payouts();
        let kernel = KernelAccount {
            market: market_id,
            phase: 0,
            basis_mode: clutch_kernel::BasisMode::FinitePreset,
            resolved_payout: 0,
            payouts: payout_set,
            total_supply: internal,
        };
        let kernel_address = plane.kernel.address;
        account_mut(&mut plane, kernel_address).data =
            encode(clutch_solana_reference::KERNEL_ACCOUNT_LEN, |out| {
                kernel.encode(out)
            });
        let supply_address = plane.supply.address;
        let mut supply = SupplyLedgerAccount::decode(&account_mut(&mut plane, supply_address).data)
            .expect("supply decodes");
        supply.outcome_count = OUTCOMES;
        supply.internal_supply = internal;
        supply.external_supply = [0; MAX_OUTCOMES];
        account_mut(&mut plane, supply_address).data =
            encode(account_len::SUPPLY_LEDGER, |out| supply.encode(out));
        let hoard_address = plane.hoard.address;
        let mut hoard = HoardAccount::decode(&account_mut(&mut plane, hoard_address).data)
            .expect("hoard decodes");
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
        let resolution_address = plane.resolution.address;
        account_mut(&mut plane, resolution_address).data =
            encode(account_len::RESOLUTION, |out| unresolved.encode(out));
    }

    let spec_pda = pda(&[seeds::SEED_SOURCE_SPEC, &feed_id.bytes()]);
    let feed_pda = pda(&[seeds::SEED_FEED, &feed_id.bytes()]);
    let window = source_window(&terms, feed_id);
    let window_id = source_archive_v2::canonical_window_id(window);
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
        grid,
        grid_value,
        grid_bytes,
        spec,
        window,
        start_bucket,
        end_bucket_exclusive,
        market_prestate,
    }
}

/// One immutable artifact upload whose stage is uploader-keyed and whose
/// final address is content-derived. The final body is never installed at
/// genesis by this campaign.
pub struct ArtifactUpload {
    pub binding: ArtifactBinding,
    pub stage: Pda,
    pub final_account: Pda,
    pub body: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerState {
    pub actor: Address,
    pub owner: Hash32,
    pub position: Pda,
    pub replay: Pda,
    pub collateral: Address,
}

pub fn owner_state(actor: Address, lab: &LabPlane) -> OwnerState {
    let (position, replay) = lab.plane.owner_plane(actor);
    OwnerState {
        actor,
        owner: Hash32::from_bytes(actor.to_bytes()),
        position,
        replay,
        collateral: actor_collateral(actor),
    }
}

pub fn price_grid_upload(actor: Address, lab: &LabPlane) -> ArtifactUpload {
    let binding = ArtifactBinding {
        kind: ArtifactKind::PriceGrid,
        context: lab.plane.realm_id,
        digest: lab.grid_value.grid,
        exact_len: u16::try_from(lab.grid_bytes.len()).expect("PriceGrid length fits u16"),
    };
    let kind = [binding.kind.byte()];
    let stage = pda(&[
        seeds::SEED_ARTIFACT_STAGE,
        actor.as_ref(),
        &kind,
        &binding.context.bytes(),
        &binding.digest.bytes(),
    ]);
    ArtifactUpload {
        binding,
        stage,
        final_account: lab.grid,
        body: lab.grid_bytes.clone(),
    }
}

fn artifact_upload(
    actor: Address,
    binding: ArtifactBinding,
    final_account: Pda,
    body: Vec<u8>,
) -> ArtifactUpload {
    let kind = [binding.kind.byte()];
    let stage = pda(&[
        seeds::SEED_ARTIFACT_STAGE,
        actor.as_ref(),
        &kind,
        &binding.context.bytes(),
        &binding.digest.bytes(),
    ]);
    ArtifactUpload {
        binding,
        stage,
        final_account,
        body,
    }
}

pub struct GeneralPlane {
    pub epoch_id: Hash32,
    pub policy_digest: Hash32,
    pub policy: ArtifactUpload,
    pub epoch: Pda,
    pub window: Pda,
    pub page: Pda,
}

pub fn general_plane(actor: Address, lab: &LabPlane) -> GeneralPlane {
    let epoch_id = canonical_epoch_id(lab.plane.market_id, GENERAL_EPOCH_INDEX);
    let policy_body = canonical_batch_policy_bytes(&GENERAL_CLEARING_POLICY_V1)
        .expect("general clearing policy encodes")
        .to_vec();
    let policy_digest = Hash32::from_bytes(
        batch_policy_digest(&GENERAL_CLEARING_POLICY_V1)
            .expect("general clearing policy digests")
            .0,
    );
    let policy_final = pda(&[
        seeds::SEED_BATCH_POLICY,
        &epoch_id.bytes(),
        &policy_digest.bytes(),
    ]);
    let policy = artifact_upload(
        actor,
        ArtifactBinding {
            kind: ArtifactKind::BatchPolicy,
            context: epoch_id,
            digest: policy_digest,
            exact_len: u16::try_from(policy_body.len()).expect("policy length fits u16"),
        },
        policy_final,
        policy_body,
    );
    let index = GENERAL_EPOCH_INDEX.to_le_bytes();
    GeneralPlane {
        epoch_id,
        policy_digest,
        policy,
        epoch: pda(&[seeds::SEED_EPOCH, &lab.plane.market_id.bytes(), &index]),
        window: pda(&[
            seeds::SEED_EPOCH_WINDOW,
            &lab.plane.market_id.bytes(),
            &index,
        ]),
        page: pda(&[seeds::SEED_PAGE, &epoch_id.bytes(), &0_u16.to_le_bytes()]),
    }
}

pub fn init_epoch(
    actor: Address,
    lab: &LabPlane,
    general: &GeneralPlane,
    freeze_deadline_slot: u64,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::InitEpoch {
                market: lab.plane.market_id,
                epoch_index: GENERAL_EPOCH_INDEX,
                policy: general.policy_digest,
                freeze_deadline_slot,
            },
        ),
        vec![
            AccountMeta::new(actor, true),
            AccountMeta::new_readonly(lab.plane.market.address, false),
            AccountMeta::new_readonly(lab.plane.terms.address, false),
            AccountMeta::new_readonly(lab.grid.address, false),
            AccountMeta::new_readonly(general.policy.final_account.address, false),
            AccountMeta::new(general.epoch.address, false),
            AccountMeta::new(general.window.address, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(
                Address::new_from_array(
                    clutch_sbf::instructions::artifact::CLOCK_SYSVAR_ID.to_bytes(),
                ),
                false,
            ),
        ],
    )
}

pub fn init_order_page(actor: Address, lab: &LabPlane, general: &GeneralPlane) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::InitOrderPage {
                market: lab.plane.market_id,
                epoch: general.epoch_id,
                page_index: 0,
                page_count: 1,
            },
        ),
        vec![
            AccountMeta::new(actor, true),
            AccountMeta::new(general.page.address, false),
            AccountMeta::new_readonly(lab.plane.market.address, false),
            AccountMeta::new_readonly(general.epoch.address, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ],
    )
}

pub struct OrderPlacement {
    pub reservation: Pda,
    pub instruction: Instruction,
}

pub fn place_single_order(
    actor: Address,
    lab: &LabPlane,
    general: &GeneralPlane,
    sequence: u64,
    side: u8,
) -> OrderPlacement {
    let owner = owner_state(actor, lab);
    let rank = sequence
        .checked_add(1)
        .expect("order rank does not overflow");
    let slot = OrderSlot::Single(OrderRecord {
        owner: owner.owner,
        order_id: canonical_order_id(rank),
        outcome: 1,
        side,
        quantity: ORDER_QUANTITY,
        limit: if side == 0 { BUY_LIMIT } else { SELL_LIMIT },
        minimum_fill: 0,
        flags: 0,
        generation: 1,
        expiry_epoch: GENERAL_EPOCH_INDEX,
    });
    let reservation_id = canonical_reservation_id(
        lab.plane.market_id,
        general.epoch_id,
        owner.owner,
        0,
        slot.order_id(),
    );
    let reservation = pda(&[seeds::SEED_RESERVATION, &reservation_id.bytes()]);
    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            sequence,
            Intent::PlaceOrder {
                market: lab.plane.market_id,
                epoch: general.epoch_id,
                max_fee_atoms: 0,
                slot,
            },
        ),
        vec![
            AccountMeta::new(actor, true),
            AccountMeta::new_readonly(general.epoch.address, false),
            AccountMeta::new_readonly(lab.grid.address, false),
            AccountMeta::new(general.page.address, false),
            AccountMeta::new(owner.position.address, false),
            AccountMeta::new(reservation.address, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ],
    );
    OrderPlacement {
        reservation,
        instruction,
    }
}

pub fn freeze_epoch(lab: &LabPlane, general: &GeneralPlane) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::FreezeEpoch {
                market: lab.plane.market_id,
                epoch: general.epoch_id,
            },
        ),
        vec![
            AccountMeta::new(general.epoch.address, false),
            AccountMeta::new(general.window.address, false),
            AccountMeta::new_readonly(
                Address::new_from_array(
                    clutch_sbf::instructions::artifact::CLOCK_SYSVAR_ID.to_bytes(),
                ),
                false,
            ),
            AccountMeta::new(general.page.address, false),
        ],
    )
}

pub struct CandidatePlan {
    pub candidate: Hash32,
    pub record: Pda,
    pub feed: Pda,
    pub work: Pda,
    pub pot: Pda,
    pub receipt: Pda,
    pub prices: [u64; MAX_OUTCOMES],
    pub fills: [u64; FEED_FILLS_PER_CHUNK],
    pub slices: [PairingSlice; FEED_SLICES_PER_CHUNK],
    pub slice_count: u16,
}

fn relation_domain(epoch: &EpochAccount) -> RelationDomainV1 {
    RelationDomainV1 {
        relation_version: epoch.relation_version,
        market_id: 0,
        book_id: 0,
        epoch: epoch.epoch_index,
        policy_id: 0,
        order_set_id: 0,
        outcome_count: epoch.outcome_count,
        owner_count: epoch.owner_count,
        price_scale: epoch.price_scale,
        remainder_seed: epoch.remainder_seed,
        policy: GENERAL_CLEARING_POLICY_V1,
    }
}

fn layout_leg(value: LegRefV1) -> LegRef {
    match value {
        LegRefV1::Order(index) => LegRef::Order(index),
        LegRefV1::Split => LegRef::Split,
        LegRefV1::Merge => LegRef::Merge,
    }
}

pub fn candidate_plan(
    lab: &LabPlane,
    general: &GeneralPlane,
    epoch: &EpochAccount,
    page_bytes: &[u8],
) -> Result<CandidatePlan, Box<dyn std::error::Error>> {
    let header = stream::OrderPageHeader::decode(page_bytes)
        .map_err(|error| format!("frozen order page header: {error:?}"))?;
    let mut cursor = stream::OrderSlotCursor::new(page_bytes)
        .map_err(|error| format!("frozen order page cursor: {error:?}"))?;
    let mut owners = OwnerInterner::NEW;
    let mut book = Box::new(BookV1::empty());
    let mut live = 0_u8;
    for _ in 0..header.order_count {
        let slot = cursor
            .next_slot()
            .ok_or("frozen page ended inside its populated prefix")?
            .map_err(|error| format!("frozen order slot: {error:?}"))?;
        if let Some(order) = project_slot(&slot, u64::from(live) + 1, &mut owners)
            .map_err(|error| format!("relation projection: {error:?}"))?
        {
            book.orders[usize::from(live)] = order;
            live = live.checked_add(1).ok_or("live order count overflow")?;
        }
    }
    book.len = live;
    if live != 2 || owners.count() != 2 {
        return Err(format!(
            "joined book expected two live orders and two owners, got {live} orders and {} owners",
            owners.count()
        )
        .into());
    }
    let prices = [
        2_500, 2_500, 2_500, 2_500, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    let domain = relation_domain(epoch);
    let candidate_v1 = canonical_candidate(&domain, &book, &prices, 0, 0)
        .map_err(|error| format!("canonical candidate: {error:?}"))?;
    if candidate_v1.fills[..2] != [ORDER_QUANTITY, ORDER_QUANTITY] {
        return Err(format!("canonical fills differ: {:?}", &candidate_v1.fills[..2]).into());
    }
    let witness = canonical_pairing(&domain, &book, &candidate_v1)
        .map_err(|error| format!("canonical pairing: {error:?}"))?;
    if witness.len != 1 {
        return Err(format!("expected one direct witness slice, got {}", witness.len).into());
    }
    let witness_slice = witness.slices[0];
    if witness_slice.buy_ref != LegRefV1::Order(0)
        || witness_slice.sell_ref != LegRefV1::Order(1)
        || witness_slice.outcome != 1
        || witness_slice.quantity != ORDER_QUANTITY
    {
        return Err(format!("unexpected canonical witness slice: {witness_slice:?}").into());
    }
    let mut record = CandidateRecord {
        candidate: Hash32::ZERO,
        epoch: general.epoch_id,
        market: lab.plane.market_id,
        prices,
        virtual_split: 0,
        virtual_merge: 0,
        honored_aon_mask: 0,
        weighted_direct_volume: 0,
        limit_surplus_price_units: 0,
        score_digest: Hash32::ZERO,
        churn: 0,
        submitted_slot: 1,
        distinct_owners: 0,
        order_len: 2,
        outcome_count: OUTCOMES,
        status: CANDIDATE_STATUS_SUBMITTED,
        stored_bump: 0,
        flags: 0,
    };
    record.candidate = record
        .recomputed_candidate_digest()
        .map_err(|error| format!("candidate identity: {error:?}"))?;
    let candidate = record.candidate;
    let record_pda = pda(&[
        seeds::SEED_CANDIDATE,
        &general.epoch_id.bytes(),
        &candidate.bytes(),
    ]);
    let feed = pda(&[
        seeds::SEED_CANDIDATE_FEED,
        &general.epoch_id.bytes(),
        &candidate.bytes(),
    ]);
    let work = pda(&[
        seeds::SEED_CLEAR_WORK,
        &general.epoch_id.bytes(),
        &candidate.bytes(),
    ]);
    let pot = pda(&[seeds::SEED_POT, &general.epoch_id.bytes()]);
    let receipt = pda(&[
        seeds::SEED_RECEIPT,
        &general.epoch_id.bytes(),
        &candidate.bytes(),
        &0_u16.to_le_bytes(),
    ]);
    let mut fills = [0_u64; FEED_FILLS_PER_CHUNK];
    fills[..2].copy_from_slice(&candidate_v1.fills[..2]);
    let mut slices = [PairingSlice::PADDING; FEED_SLICES_PER_CHUNK];
    slices[0] = PairingSlice {
        buy_ref: layout_leg(witness_slice.buy_ref),
        sell_ref: layout_leg(witness_slice.sell_ref),
        outcome: witness_slice.outcome,
        quantity: witness_slice.quantity,
    };
    Ok(CandidatePlan {
        candidate,
        record: record_pda,
        feed,
        work,
        pot,
        receipt,
        prices,
        fills,
        slices,
        slice_count: witness.len,
    })
}

pub fn submit_candidate(
    actor: Address,
    lab: &LabPlane,
    general: &GeneralPlane,
    candidate: &CandidatePlan,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::SubmitCandidate {
                market: lab.plane.market_id,
                epoch: general.epoch_id,
                prices: candidate.prices,
                virtual_split: 0,
                virtual_merge: 0,
                honored_aon_mask: 0,
                declared_slices: Some(candidate.slice_count),
                weighted_direct_volume: 0,
                limit_surplus_price_units: 0,
                distinct_owners: 0,
            },
        ),
        vec![
            AccountMeta::new(actor, true),
            AccountMeta::new_readonly(general.epoch.address, false),
            AccountMeta::new_readonly(general.window.address, false),
            AccountMeta::new(candidate.record.address, false),
            AccountMeta::new(candidate.feed.address, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(
                Address::new_from_array(
                    clutch_sbf::instructions::artifact::CLOCK_SYSVAR_ID.to_bytes(),
                ),
                false,
            ),
        ],
    )
}

pub fn write_candidate_fills(
    lab: &LabPlane,
    general: &GeneralPlane,
    candidate: &CandidatePlan,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::WriteCandidateFeed {
                market: lab.plane.market_id,
                epoch: general.epoch_id,
                candidate: candidate.candidate,
                chunk: CandidateFeedChunk::Fills {
                    count: 2,
                    fills: candidate.fills,
                },
            },
        ),
        vec![AccountMeta::new(candidate.feed.address, false)],
    )
}

pub fn write_candidate_slices(
    lab: &LabPlane,
    general: &GeneralPlane,
    candidate: &CandidatePlan,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            2,
            Intent::WriteCandidateFeed {
                market: lab.plane.market_id,
                epoch: general.epoch_id,
                candidate: candidate.candidate,
                chunk: CandidateFeedChunk::Slices {
                    count: u8::try_from(candidate.slice_count).expect("slice count fits u8"),
                    slices: candidate.slices,
                },
            },
        ),
        vec![AccountMeta::new(candidate.feed.address, false)],
    )
}

pub fn seal_candidate(
    lab: &LabPlane,
    general: &GeneralPlane,
    candidate: &CandidatePlan,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::SealCandidate {
                market: lab.plane.market_id,
                epoch: general.epoch_id,
                candidate: candidate.candidate,
            },
        ),
        vec![
            AccountMeta::new_readonly(general.epoch.address, false),
            AccountMeta::new_readonly(general.window.address, false),
            AccountMeta::new(candidate.feed.address, false),
            AccountMeta::new_readonly(
                Address::new_from_array(
                    clutch_sbf::instructions::artifact::CLOCK_SYSVAR_ID.to_bytes(),
                ),
                false,
            ),
        ],
    )
}

pub fn init_clear_work(
    actor: Address,
    lab: &LabPlane,
    general: &GeneralPlane,
    candidate: &CandidatePlan,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::InitClearWork {
                market: lab.plane.market_id,
                epoch: general.epoch_id,
                candidate: candidate.candidate,
            },
        ),
        vec![
            AccountMeta::new(actor, true),
            AccountMeta::new(candidate.work.address, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ],
    )
}

pub fn grow_clear_work(
    lab: &LabPlane,
    general: &GeneralPlane,
    candidate: &CandidatePlan,
    sequence: u64,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            sequence,
            Intent::GrowClearWork {
                market: lab.plane.market_id,
                epoch: general.epoch_id,
                candidate: candidate.candidate,
            },
        ),
        vec![AccountMeta::new(candidate.work.address, false)],
    )
}

pub fn advance_clear_work(
    lab: &LabPlane,
    general: &GeneralPlane,
    candidate: &CandidatePlan,
    reservations: &[Pda],
) -> Instruction {
    let mut metas = vec![
        AccountMeta::new_readonly(general.epoch.address, false),
        AccountMeta::new_readonly(candidate.feed.address, false),
        AccountMeta::new(candidate.work.address, false),
        AccountMeta::new_readonly(general.page.address, false),
    ];
    metas.extend(
        reservations
            .iter()
            .map(|reservation| AccountMeta::new_readonly(reservation.address, false)),
    );
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::AdvanceClearWork {
                market: lab.plane.market_id,
                epoch: general.epoch_id,
                candidate: candidate.candidate,
                max_orders: 16,
            },
        ),
        metas,
    )
}

pub fn advance_clear_slices(
    lab: &LabPlane,
    general: &GeneralPlane,
    candidate: &CandidatePlan,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::AdvanceClearSlices {
                market: lab.plane.market_id,
                epoch: general.epoch_id,
                candidate: candidate.candidate,
                max_slices: candidate.slice_count,
            },
        ),
        vec![
            AccountMeta::new_readonly(general.epoch.address, false),
            AccountMeta::new_readonly(candidate.feed.address, false),
            AccountMeta::new(candidate.work.address, false),
        ],
    )
}

pub fn complete_clear_work(
    lab: &LabPlane,
    general: &GeneralPlane,
    candidate: &CandidatePlan,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::CompleteClearWork {
                market: lab.plane.market_id,
                epoch: general.epoch_id,
                candidate: candidate.candidate,
            },
        ),
        vec![
            AccountMeta::new_readonly(general.epoch.address, false),
            AccountMeta::new_readonly(candidate.feed.address, false),
            AccountMeta::new(candidate.work.address, false),
            AccountMeta::new(candidate.record.address, false),
            AccountMeta::new(general.window.address, false),
            AccountMeta::new_readonly(
                Address::new_from_array(
                    clutch_sbf::instructions::artifact::CLOCK_SYSVAR_ID.to_bytes(),
                ),
                false,
            ),
        ],
    )
}

pub fn finalize_selection(
    lab: &LabPlane,
    general: &GeneralPlane,
    candidate: &CandidatePlan,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::FinalizeSelection {
                market: lab.plane.market_id,
                epoch: general.epoch_id,
            },
        ),
        vec![
            AccountMeta::new(general.epoch.address, false),
            AccountMeta::new(general.window.address, false),
            AccountMeta::new_readonly(
                Address::new_from_array(
                    clutch_sbf::instructions::artifact::CLOCK_SYSVAR_ID.to_bytes(),
                ),
                false,
            ),
            AccountMeta::new(candidate.record.address, false),
            AccountMeta::new_readonly(candidate.feed.address, false),
        ],
    )
}

pub fn freeze_entitlement(
    actor: Address,
    lab: &LabPlane,
    general: &GeneralPlane,
    candidate: &CandidatePlan,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::FreezeEntitlement {
                market: lab.plane.market_id,
                epoch: general.epoch_id,
                candidate: candidate.candidate,
            },
        ),
        vec![
            AccountMeta::new(actor, true),
            AccountMeta::new_readonly(general.epoch.address, false),
            AccountMeta::new_readonly(candidate.record.address, false),
            AccountMeta::new_readonly(candidate.work.address, false),
            AccountMeta::new(candidate.pot.address, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ],
    )
}

pub fn entitle_slice(
    actor: Address,
    lab: &LabPlane,
    general: &GeneralPlane,
    candidate: &CandidatePlan,
    buyer_reservation: Pda,
    seller_reservation: Pda,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::EntitleSlice {
                market: lab.plane.market_id,
                epoch: general.epoch_id,
                candidate: candidate.candidate,
                slice_index: 0,
            },
        ),
        vec![
            AccountMeta::new(actor, true),
            AccountMeta::new_readonly(general.epoch.address, false),
            AccountMeta::new_readonly(candidate.record.address, false),
            AccountMeta::new_readonly(candidate.feed.address, false),
            AccountMeta::new_readonly(candidate.pot.address, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(general.page.address, false),
            AccountMeta::new(buyer_reservation.address, false),
            AccountMeta::new(seller_reservation.address, false),
            AccountMeta::new(candidate.receipt.address, false),
        ],
    )
}

pub fn settle_slice(
    buyer: Address,
    seller: Address,
    lab: &LabPlane,
    general: &GeneralPlane,
    candidate: &CandidatePlan,
    buyer_reservation: Pda,
    seller_reservation: Pda,
) -> Instruction {
    let buyer = owner_state(buyer, lab);
    let seller = owner_state(seller, lab);
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            1,
            Intent::SettlePage {
                market: lab.plane.market_id,
                epoch: general.epoch_id,
                page_index: 0,
            },
        ),
        vec![
            AccountMeta::new_readonly(general.epoch.address, false),
            AccountMeta::new_readonly(candidate.record.address, false),
            AccountMeta::new(buyer.position.address, false),
            AccountMeta::new(seller.position.address, false),
            AccountMeta::new(buyer_reservation.address, false),
            AccountMeta::new(seller_reservation.address, false),
            AccountMeta::new(candidate.receipt.address, false),
        ],
    )
}

pub fn begin_artifact(actor: Address, upload: &ArtifactUpload, expires_slot: u64) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::BeginArtifact {
                kind: upload.binding.kind,
                context: upload.binding.context,
                digest: upload.binding.digest,
                exact_len: upload.binding.exact_len,
                expires_slot,
            },
        ),
        vec![
            AccountMeta::new(actor, true),
            AccountMeta::new(upload.stage.address, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(
                Address::new_from_array(
                    clutch_sbf::instructions::artifact::CLOCK_SYSVAR_ID.to_bytes(),
                ),
                false,
            ),
        ],
    )
}

pub fn write_artifact(
    actor: Address,
    upload: &ArtifactUpload,
    cursor: u16,
    bytes: &[u8],
) -> Instruction {
    assert!(bytes.len() <= ARTIFACT_CHUNK_BYTES);
    let mut chunk = [0_u8; ARTIFACT_CHUNK_BYTES];
    chunk[..bytes.len()].copy_from_slice(bytes);
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::WriteArtifact {
                kind: upload.binding.kind,
                context: upload.binding.context,
                digest: upload.binding.digest,
                cursor,
                chunk_len: u16::try_from(bytes.len()).expect("artifact chunk fits u16"),
                chunk,
            },
        ),
        vec![
            AccountMeta::new(actor, true),
            AccountMeta::new(upload.stage.address, false),
            AccountMeta::new_readonly(
                Address::new_from_array(
                    clutch_sbf::instructions::artifact::CLOCK_SYSVAR_ID.to_bytes(),
                ),
                false,
            ),
        ],
    )
}

pub fn seal_artifact(actor: Address, upload: &ArtifactUpload) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::SealArtifact {
                kind: upload.binding.kind,
                context: upload.binding.context,
                digest: upload.binding.digest,
                exact_len: upload.binding.exact_len,
            },
        ),
        vec![
            AccountMeta::new(actor, true),
            AccountMeta::new(upload.stage.address, false),
            AccountMeta::new(upload.final_account.address, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(
                Address::new_from_array(
                    clutch_sbf::instructions::artifact::CLOCK_SYSVAR_ID.to_bytes(),
                ),
                false,
            ),
        ],
    )
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

pub fn create_market(actor: Address, lab: &LabPlane) -> Instruction {
    assert_eq!(lab.market_prestate, MarketPrestate::SignedCreate);
    let mut metas = vec![
        AccountMeta::new(actor, true),
        AccountMeta::new_readonly(lab.plane.realm.address, false),
        AccountMeta::new_readonly(lab.plane.profile.address, false),
        AccountMeta::new_readonly(lab.plane.terms.address, false),
        AccountMeta::new(lab.plane.market.address, false),
        AccountMeta::new(lab.plane.hoard.address, false),
        AccountMeta::new(lab.plane.position.address, false),
        AccountMeta::new(lab.plane.kernel.address, false),
        AccountMeta::new(lab.plane.replay.address, false),
        AccountMeta::new(lab.plane.supply.address, false),
        AccountMeta::new(lab.plane.resolution.address, false),
        AccountMeta::new_readonly(lab.plane.policy_account, false),
        AccountMeta::new_readonly(TOKEN_2022, false),
        AccountMeta::new_readonly(COLLATERAL_MINT, false),
        AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
        AccountMeta::new_readonly(RENT_SYSVAR, false),
        AccountMeta::new_readonly(lab.plane.hoard_authority.address, false),
        AccountMeta::new(lab.plane.hoard_token.address, false),
    ];
    metas.extend(
        lab.plane
            .outcome_mints
            .iter()
            .map(|mint| AccountMeta::new(mint.address, false)),
    );
    assert_eq!(metas.len(), market_init::account_count(OUTCOMES));
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::CreateMarket {
                realm: lab.plane.realm_id,
                profile: lab.plane.profile_id,
                market_nonce: MARKET_NONCE,
                outcome_count: OUTCOMES,
                terms: lab.plane.terms_id,
                feed: lab.plane.feed_id,
            },
        ),
        metas,
    )
}

pub fn endow(actor: Address, lab: &LabPlane, sequence: u64, amount: u64) -> Instruction {
    let owner = owner_state(actor, lab);
    let metas = vec![
        AccountMeta::new(actor, true),
        AccountMeta::new_readonly(lab.plane.market.address, false),
        AccountMeta::new_readonly(lab.plane.hoard.address, false),
        AccountMeta::new(owner.position.address, false),
        AccountMeta::new(owner.replay.address, false),
        AccountMeta::new_readonly(lab.plane.profile.address, false),
        AccountMeta::new_readonly(lab.plane.policy_account, false),
        AccountMeta::new_readonly(TOKEN_2022, false),
        AccountMeta::new_readonly(COLLATERAL_MINT, false),
        AccountMeta::new(owner.collateral, false),
        AccountMeta::new(lab.plane.hoard_token.address, false),
        AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
        AccountMeta::new_readonly(RENT_SYSVAR, false),
        AccountMeta::new_readonly(lab.plane.terms.address, false),
        AccountMeta::new_readonly(lab.plane.source_spec.address, false),
    ];
    assert_eq!(metas.len(), genesis::ENDOW_ACCOUNT_COUNT);
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            sequence,
            Intent::Endow {
                market: lab.plane.market_id,
                owner: owner.owner,
                amount,
            },
        ),
        metas,
    )
}

pub fn split(actor: Address, lab: &LabPlane, sequence: u64, quantity: u64) -> Instruction {
    let owner = owner_state(actor, lab);
    let mut metas = vec![
        AccountMeta::new_readonly(actor, true),
        AccountMeta::new_readonly(lab.plane.realm.address, false),
        AccountMeta::new_readonly(lab.plane.profile.address, false),
        AccountMeta::new(lab.plane.market.address, false),
        AccountMeta::new(lab.plane.hoard.address, false),
        AccountMeta::new(owner.position.address, false),
        AccountMeta::new(lab.plane.kernel.address, false),
        AccountMeta::new(owner.replay.address, false),
        AccountMeta::new(lab.plane.supply.address, false),
        AccountMeta::new_readonly(TOKEN_2022, false),
        AccountMeta::new_readonly(lab.plane.policy_account, false),
        AccountMeta::new_readonly(COLLATERAL_MINT, false),
        AccountMeta::new(owner.collateral, false),
        AccountMeta::new_readonly(lab.plane.hoard_authority.address, false),
        AccountMeta::new(lab.plane.hoard_token.address, false),
    ];
    metas.extend(
        lab.plane
            .outcome_mints
            .iter()
            .map(|mint| AccountMeta::new_readonly(mint.address, false)),
    );
    assert_eq!(
        metas.len(),
        seam::ACCOUNT_PREFIX_COLLATERAL + usize::from(OUTCOMES)
    );
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            sequence,
            Intent::Split {
                market: lab.plane.market_id,
                owner: owner.owner,
                quantity,
            },
        ),
        metas,
    )
}

pub fn append(lab: &LabPlane, sequence: u64, update: Address, config: Address) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            sequence,
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

pub fn seal(lab: &LabPlane, sequence: u64) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            sequence,
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

pub fn redeem_internal(
    actor: Address,
    lab: &LabPlane,
    sequence: u64,
    outcome: u8,
    quantity: u64,
) -> Instruction {
    let owner = owner_state(actor, lab);
    let mut data = vec![0xd1, 1];
    data.extend_from_slice(&sequence.to_le_bytes());
    data.push(2);
    data.push(outcome);
    data.extend_from_slice(&quantity.to_le_bytes());
    let mut metas = vec![
        AccountMeta::new_readonly(actor, true),
        AccountMeta::new(lab.plane.market.address, false),
        AccountMeta::new(lab.plane.hoard.address, false),
        AccountMeta::new(owner.position.address, false),
        AccountMeta::new(lab.plane.kernel.address, false),
        AccountMeta::new(owner.replay.address, false),
        AccountMeta::new(lab.plane.supply.address, false),
        AccountMeta::new_readonly(lab.plane.terms.address, false),
        AccountMeta::new_readonly(lab.plane.resolution.address, false),
        AccountMeta::new_readonly(lab.plane.profile.address, false),
        AccountMeta::new_readonly(TOKEN_2022, false),
        AccountMeta::new_readonly(lab.plane.policy_account, false),
        AccountMeta::new_readonly(COLLATERAL_MINT, false),
        AccountMeta::new(owner.collateral, false),
        AccountMeta::new_readonly(lab.plane.hoard_authority.address, false),
        AccountMeta::new(lab.plane.hoard_token.address, false),
    ];
    metas.extend(
        lab.plane
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

pub fn withdraw(actor: Address, lab: &LabPlane, sequence: u64, amount: u64) -> Instruction {
    let owner = owner_state(actor, lab);
    let metas = vec![
        AccountMeta::new_readonly(actor, true),
        AccountMeta::new_readonly(lab.plane.market.address, false),
        AccountMeta::new_readonly(lab.plane.hoard.address, false),
        AccountMeta::new(owner.position.address, false),
        AccountMeta::new(owner.replay.address, false),
        AccountMeta::new_readonly(lab.plane.profile.address, false),
        AccountMeta::new_readonly(lab.plane.policy_account, false),
        AccountMeta::new_readonly(TOKEN_2022, false),
        AccountMeta::new_readonly(COLLATERAL_MINT, false),
        AccountMeta::new(owner.collateral, false),
        AccountMeta::new_readonly(lab.plane.hoard_authority.address, false),
        AccountMeta::new(lab.plane.hoard_token.address, false),
    ];
    assert_eq!(metas.len(), cash_exit::ACCOUNT_COUNT);
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            sequence,
            Intent::WithdrawCash {
                market: lab.plane.market_id,
                owner: owner.owner,
                destination: Hash32::from_bytes(owner.collateral.to_bytes()),
                amount,
            },
        ),
        metas,
    )
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

pub fn decode_position(data: &[u8]) -> Result<PositionAccount, Box<dyn std::error::Error>> {
    PositionAccount::decode(data).map_err(|error| format!("position: {error:?}").into())
}

pub fn decode_hoard(data: &[u8]) -> Result<HoardAccount, Box<dyn std::error::Error>> {
    HoardAccount::decode(data).map_err(|error| format!("hoard: {error:?}").into())
}

pub fn decode_supply(data: &[u8]) -> Result<SupplyLedgerAccount, Box<dyn std::error::Error>> {
    SupplyLedgerAccount::decode(data).map_err(|error| format!("supply: {error:?}").into())
}

pub fn token_program() -> Address {
    TOKEN_2022
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signed_plane(actor: Address) -> LabPlane {
        build(
            actor,
            real_spec().unwrap(),
            29_790_527,
            29_790_528,
            MARKET_NONCE,
            MarketPrestate::SignedCreate,
        )
    }

    #[test]
    fn joined_plane_accepts_a_two_boundary_source_window() {
        let actor = Address::new_from_array([0xa0; 32]);
        let lab = build(
            actor,
            real_spec().unwrap(),
            29_790_526,
            29_790_528,
            MARKET_NONCE,
            MarketPrestate::SignedCreate,
        );
        let terms = TermsAccount::decode(
            &lab.plane
                .accounts
                .iter()
                .find(|account| account.address == lab.plane.terms.address)
                .expect("Terms fixture exists")
                .data,
        )
        .expect("Terms fixture decodes");
        assert_eq!(terms.expected_start_bucket, 29_790_526);
        assert_eq!(terms.expected_end_bucket_exclusive, 29_790_528);
        assert_eq!(terms.maturity_horizon_buckets, 3);
        assert_eq!(lab.window.maturity_bucket_exclusive(), 29_790_529);
        assert_eq!(lab.start_bucket, 29_790_526);
        assert_eq!(lab.end_bucket_exclusive, 29_790_528);
    }

    #[test]
    fn joined_plane_keeps_prerequisites_but_not_market_targets() {
        let actor = Address::new_from_array([0xa1; 32]);
        let lab = signed_plane(actor);
        let addresses = lab
            .plane
            .accounts
            .iter()
            .map(|account| account.address)
            .collect::<Vec<_>>();
        assert!(addresses.contains(&lab.plane.realm.address));
        assert!(addresses.contains(&lab.plane.profile.address));
        assert!(addresses.contains(&lab.plane.terms.address));
        assert!(!addresses.contains(&lab.grid.address));
        let terms = TermsAccount::decode(
            &lab.plane
                .accounts
                .iter()
                .find(|account| account.address == lab.plane.terms.address)
                .expect("Terms fixture exists")
                .data,
        )
        .expect("Terms fixture decodes");
        assert_eq!(lab.grid_bytes.len(), account_len::PRICE_GRID);
        assert_eq!(
            PriceGridAccount::decode(&lab.grid_bytes),
            Ok(lab.grid_value)
        );
        assert_eq!(lab.grid_value.binds_terms(&terms), Ok(()));
        for target in lab
            .plane
            .market_state_addresses()
            .iter()
            .chain(core::iter::once(&lab.plane.hoard_token.address))
            .chain(lab.plane.outcome_mints.iter().map(|mint| &mint.address))
        {
            assert!(!addresses.contains(target), "{target} was genesis-assisted");
        }
    }

    #[test]
    fn joined_instruction_family_is_signed_only_by_ephemeral_actor() {
        let actor = Address::new_from_array([0xa2; 32]);
        let lab = signed_plane(actor);
        for instruction in [
            create_market(actor, &lab),
            endow(actor, &lab, 0, USER_COLLATERAL_ATOMS),
            split(actor, &lab, 1, USER_COLLATERAL_ATOMS),
            redeem_internal(actor, &lab, 2, 0, USER_COLLATERAL_ATOMS),
            withdraw(actor, &lab, 6, USER_COLLATERAL_ATOMS),
        ] {
            assert_eq!(instruction.program_id, PROGRAM_ID);
            let signers = instruction
                .accounts
                .iter()
                .filter(|meta| meta.is_signer)
                .map(|meta| meta.pubkey)
                .collect::<Vec<_>>();
            assert_eq!(signers, [actor]);
        }
    }

    #[test]
    fn source_append_carries_the_archive_record_sequence() {
        let actor = Address::new_from_array([0xa2; 32]);
        let lab = signed_plane(actor);
        let instruction = append(
            &lab,
            1,
            Address::new_from_array([0xb1; 32]),
            Address::new_from_array([0xb2; 32]),
        );
        let request = clutch_solana_reference::Request::decode(&instruction.data)
            .expect("append request decodes");
        assert_eq!(request.sequence, 1);
        assert!(matches!(
            request.action,
            clutch_solana_reference::Action::Layout(Intent::AppendSourceArchiveV2 { .. })
        ));
    }

    #[test]
    fn source_seal_carries_the_final_archive_record_count() {
        let actor = Address::new_from_array([0xa2; 32]);
        let lab = signed_plane(actor);
        let instruction = seal(&lab, 2);
        let request = clutch_solana_reference::Request::decode(&instruction.data)
            .expect("seal request decodes");
        assert_eq!(request.sequence, 2);
        assert!(matches!(
            request.action,
            clutch_solana_reference::Action::Layout(Intent::SealSourceArchiveV2 { .. })
        ));
    }

    #[test]
    fn joined_user_token_address_is_deterministic_and_actor_scoped() {
        let first = Address::new_from_array([0xa3; 32]);
        let second = Address::new_from_array([0xa4; 32]);
        assert_eq!(actor_collateral(first), actor_collateral(first));
        assert_ne!(actor_collateral(first), actor_collateral(second));
    }

    #[test]
    fn general_candidate_builder_reuses_the_canonical_relation_and_pairing() {
        let buyer = Address::new_from_array([0xb1; 32]);
        let seller = Address::new_from_array([0xb2; 32]);
        let lab = signed_plane(buyer);
        let general = general_plane(buyer, &lab);
        let mut epoch = clutch_solana_layout::clearing::open_general_epoch(
            lab.plane.market_id,
            lab.plane.terms_id,
            lab.grid_value.grid,
            general.policy_digest,
            GENERAL_EPOCH_INDEX,
            PRICE_SCALE,
            OUTCOMES,
            0,
            general.epoch.bump,
        )
        .expect("general epoch opens");
        epoch.phase = clutch_solana_layout::EPOCH_PHASE_FROZEN;
        epoch.owner_count = 2;
        epoch.page_count = 1;
        epoch.order_count = 2;

        let mut page = vec![0_u8; account_len::ORDER_PAGE];
        stream::init_page(
            &mut page,
            lab.plane.market_id,
            general.epoch_id,
            0,
            1,
            general.page.bump,
        )
        .expect("page initializes");
        for (rank, owner, side, limit) in
            [(1_u64, buyer, 0_u8, BUY_LIMIT), (2, seller, 1, SELL_LIMIT)]
        {
            stream::append_slot(
                &mut page,
                OrderSlot::Single(OrderRecord {
                    owner: Hash32::from_bytes(owner.to_bytes()),
                    order_id: canonical_order_id(rank),
                    outcome: 1,
                    side,
                    quantity: ORDER_QUANTITY,
                    limit,
                    minimum_fill: 0,
                    flags: 0,
                    generation: 1,
                    expiry_epoch: GENERAL_EPOCH_INDEX,
                }),
            )
            .expect("order appends");
        }
        let (order_set, count) =
            stream::frozen_set_commitment(&[page.as_slice()]).expect("page set commits");
        stream::seal_page(&mut page, order_set, count).expect("page seals");
        epoch.order_set = order_set;
        let header = stream::OrderPageHeader::decode(&page).expect("sealed header decodes");
        epoch.first_order_id = header.first_order_id;
        epoch.last_order_id = header.last_order_id;

        let candidate = candidate_plan(&lab, &general, &epoch, &page)
            .expect("canonical joined candidate derives");
        assert_eq!(candidate.fills[..2], [ORDER_QUANTITY, ORDER_QUANTITY]);
        assert_eq!(candidate.slice_count, 1);
        assert_eq!(candidate.slices[0].buy_ref, LegRef::Order(0));
        assert_eq!(candidate.slices[0].sell_ref, LegRef::Order(1));
        assert_eq!(candidate.slices[0].outcome, 1);
        assert_eq!(candidate.slices[0].quantity, ORDER_QUANTITY);
    }
}
