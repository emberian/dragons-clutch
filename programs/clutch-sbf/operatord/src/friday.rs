//! The Friday clutch: an eight-outcome degree-1 market, founded for real.
//!
//! The terms artifact is the disagreement exhibit's T0 construction
//! (`svm-tests/tests/disagreement_exhibit.rs::degree1_terms`) — `basis_degree`
//! 1, `knot_count` 8, u128 cent knots $100..$240 step $20, general spacing
//! (`UNIFORM_SPACING_NONE`, admitted at degree 1), STAT-TERMINAL-01,
//! EDGE-CLAMP-01, payout map entirely unused, one uniform 8/64 failure-refund
//! preset.  The exhibit *injects* its market, its positions and its Egg
//! balances as laboratory bank state; this session does not.  Here the market
//! is created by a signed `CreateMarket`, the cash arrives by a signed
//! `Endow`, and the Eggs arrive by a signed `Split`, because the whole point
//! of the trade bench is that a person drives those transitions.
//!
//! What *is* genesis-assisted, and stated as such on the banner: the frozen
//! Realm prerequisites an ordinary wallet cannot author — Realm, Profile,
//! collateral policy, the Token-2022 collateral mint, the two traders'
//! ordinary collateral accounts, this market's price grid and terms, and the
//! epoch's frozen batch policy.  Every one is program-owned or Token-owned
//! state, installed before slot zero, exactly as the general-clearing lane
//! installs its six.
//!
//! ## The price ladder
//!
//! A single-Egg limit is admitted only when it is an exact member of the
//! frozen tick vector (`PriceGridAccount::tick_of`).  The ladder here is
//! uniform — every multiple of [`LADDER_STEP`] from zero to the price scale,
//! fifty-one ticks — so that *any* belief quantized in ladder units is
//! quotable, and the midpoint of two such beliefs is a price vector on the
//! simplex with every crossing pair still eligible.  A non-uniform ladder
//! carrying only the exhibit's ten limits would admit the exhibit's book and
//! nothing a person painted.

use clutch_sbf::seeds;
use clutch_sbf_harness::{base58_of, derive, Pda, Shared};
use clutch_solana_layout::{
    account_len, canonical_epoch_id, canonical_market_id, Hash32, PayoutVectorBytes,
    PriceGridAccount, TermsAccount, MAX_GRID_TICKS, MAX_KNOTS, MAX_OUTCOMES, MAX_PAYOUTS,
    PAYOUT_MAP_UNUSED, UNIFORM_SPACING_NONE,
};

/// The Friday clutch's active width: eight hats on the knot grid.
pub const OUTCOMES: u8 = 8;
/// The market nonce this session founds under, distinct from every nonce the
/// committed lanes reserve.
pub const NONCE_FRIDAY: u64 = 71;
/// The epoch index the session trades.
pub const EPOCH_INDEX: u64 = 1;
/// The frozen price scale.
pub const PRICE_SCALE: u64 = 10_000;
/// The uniform tick spacing of the frozen limit ladder.
pub const LADDER_STEP: u64 = 200;
/// The eight knots, in u128 cents: $100..$240 step $20.
pub const KNOT_CENTS: [u128; 8] = [
    10_000, 12_000, 14_000, 16_000, 18_000, 20_000, 22_000, 24_000,
];
/// The market's collateral cap, in atoms.
pub const COLLATERAL_CAP: u64 = 1_000_000;
/// Collateral atoms each trader's ordinary token account holds at genesis.
pub const WALLET_ATOMS: u64 = 100_000;

/// One genesis row, as `solana-test-validator --account` reads it.
pub struct Row {
    pub role: String,
    pub address: String,
    pub owner: String,
    pub data: Vec<u8>,
}

/// One signing participant of the session.
pub struct Actor {
    /// `human` or `bot`; the browser's own vocabulary for the two.
    pub role: &'static str,
    pub label: &'static str,
    pub key: [u8; 32],
    pub id: Hash32,
    pub position: Pda,
    pub replay: Pda,
    pub token: Pda,
}

/// Every address the Friday session touches, and the artifacts it installs.
pub struct Friday {
    pub shared: Shared,
    pub market_id: Hash32,
    pub epoch_id: Hash32,
    pub policy_digest: Hash32,
    pub terms_value: TermsAccount,
    pub grid_value: PriceGridAccount,
    pub terms: Pda,
    pub grid: Pda,
    pub batch_policy: Pda,
    pub market: Pda,
    pub hoard: Pda,
    pub kernel: Pda,
    pub supply: Pda,
    pub resolution: Pda,
    pub hoard_authority: Pda,
    pub hoard_token: Pda,
    pub outcome_mints: Vec<Pda>,
    pub epoch: Pda,
    pub window: Pda,
    pub page: Pda,
    pub pot: Pda,
    pub actors: Vec<Actor>,
}

fn encode<const N: usize, F>(writer: F) -> Vec<u8>
where
    F: FnOnce(&mut [u8]) -> Result<usize, clutch_solana_layout::CodecError>,
{
    let mut bytes = vec![0_u8; N];
    let written = writer(&mut bytes).expect("a Friday genesis artifact encodes");
    assert_eq!(written, N, "a genesis artifact wrote a short image");
    bytes
}

/// The frozen limit ladder: every multiple of [`LADDER_STEP`] up to the scale.
fn ladder(realm: Hash32) -> PriceGridAccount {
    let count = (PRICE_SCALE / LADDER_STEP) as usize + 1;
    assert!(count <= MAX_GRID_TICKS, "the ladder must fit the grid");
    let mut ticks = [0_u64; MAX_GRID_TICKS];
    for (index, tick) in ticks.iter_mut().take(count).enumerate() {
        *tick = index as u64 * LADDER_STEP;
    }
    PriceGridAccount {
        grid: Hash32::ZERO,
        realm,
        price_scale: PRICE_SCALE,
        tick_count: u8::try_from(count).expect("the ladder is under 64 ticks"),
        ticks,
        stored_bump: 0,
        flags: 0,
    }
}

/// The exhibit's degree-1 terms, over this session's Realm and price grid.
fn degree1_terms(shared: &Shared, grid: Hash32) -> TermsAccount {
    let mut terms = shared.terms_account;
    let mut weights = [0_u64; MAX_OUTCOMES];
    for weight in weights.iter_mut().take(OUTCOMES as usize) {
        *weight = 8;
    }
    let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
    payouts[0] = PayoutVectorBytes {
        denominator: 64,
        weights,
    };
    let mut knots = [0_u128; MAX_KNOTS];
    knots[..8].copy_from_slice(&KNOT_CENTS);
    terms.price_grid = grid;
    terms.outcome_count = OUTCOMES;
    terms.payout_count = 1;
    terms.payouts = payouts;
    terms.payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
    terms.basis_degree = 1;
    terms.knot_count = 8;
    terms.knots = knots;
    terms.uniform_log2_spacing = UNIFORM_SPACING_NONE;
    terms.failure_payout_index = 0;
    terms.statistic_id = 1; // STAT-TERMINAL-01
    terms.edge_policy_id = 1; // EDGE-CLAMP-01
    terms.collateral_cap = COLLATERAL_CAP;
    terms.stored_bump = 0;
    terms
}

impl Friday {
    /// Derive every address and build every artifact this session installs.
    ///
    /// `shared` supplies the Realm, Profile, collateral policy and mint the
    /// committed lanes already use — the artifacts that are about the *venue*
    /// rather than about this market — and the two ephemeral wallet keys the
    /// daemon minted.
    #[allow(clippy::too_many_lines)] // one derivation per address, in one place
    pub fn build(shared: Shared) -> Self {
        let pid = shared.program.address.clone();
        let mut grid_value = ladder(shared.realm_hash);
        grid_value.grid = grid_value
            .recomputed_grid_id()
            .expect("the Friday ladder digests");
        let grid = derive(
            &pid,
            &[
                seeds::SEED_GRID.to_vec(),
                shared.realm_hash.bytes().to_vec(),
                grid_value.grid.bytes().to_vec(),
            ],
        );
        grid_value.stored_bump = grid.bump;

        let mut terms_value = degree1_terms(&shared, grid_value.grid);
        terms_value.terms = terms_value
            .recomputed_terms_digest()
            .expect("the degree-1 terms body digests");
        let terms = derive(
            &pid,
            &[
                seeds::SEED_TERMS.to_vec(),
                shared.realm_hash.bytes().to_vec(),
                terms_value.terms.bytes().to_vec(),
            ],
        );
        terms_value.stored_bump = terms.bump;
        terms_value
            .validate()
            .expect("the degree-1 terms artifact validates");
        grid_value
            .binds_terms(&terms_value)
            .expect("the ladder binds the degree-1 terms");

        let market_id = canonical_market_id(shared.realm_hash, shared.profile_hash, NONCE_FRIDAY);
        let epoch_id = canonical_epoch_id(market_id, EPOCH_INDEX);
        let market_seed = market_id.bytes().to_vec();
        let epoch_seed = epoch_id.bytes().to_vec();

        let market = derive(
            &pid,
            &[
                seeds::SEED_MARKET.to_vec(),
                shared.realm_hash.bytes().to_vec(),
                market_seed.clone(),
            ],
        );
        let hoard = derive(&pid, &[seeds::SEED_HOARD.to_vec(), market_seed.clone()]);
        let kernel = derive(&pid, &[seeds::SEED_KERNEL.to_vec(), market_seed.clone()]);
        let supply = derive(&pid, &[seeds::SEED_SUPPLY.to_vec(), market_seed.clone()]);
        let resolution = derive(
            &pid,
            &[seeds::SEED_RESOLUTION.to_vec(), market_seed.clone()],
        );
        let hoard_authority = derive(
            &pid,
            &[seeds::SEED_HOARD_AUTHORITY.to_vec(), market_seed.clone()],
        );
        let hoard_token = derive(
            &pid,
            &[seeds::SEED_HOARD_TOKEN.to_vec(), market_seed.clone()],
        );
        let outcome_mints: Vec<Pda> = (0..OUTCOMES)
            .map(|outcome| {
                derive(
                    &pid,
                    &[
                        seeds::SEED_OUTCOME_MINT.to_vec(),
                        market_seed.clone(),
                        vec![outcome],
                    ],
                )
            })
            .collect();

        let epoch = derive(
            &pid,
            &[
                seeds::SEED_EPOCH.to_vec(),
                market_seed.clone(),
                EPOCH_INDEX.to_le_bytes().to_vec(),
            ],
        );
        let window = derive(
            &pid,
            &[
                seeds::SEED_EPOCH_WINDOW.to_vec(),
                market_seed.clone(),
                EPOCH_INDEX.to_le_bytes().to_vec(),
            ],
        );
        let page = derive(
            &pid,
            &[
                seeds::SEED_PAGE.to_vec(),
                epoch_seed.clone(),
                0_u16.to_le_bytes().to_vec(),
            ],
        );
        let pot = derive(&pid, &[seeds::SEED_POT.to_vec(), epoch_seed.clone()]);

        let policy_digest = Hash32::from_bytes(
            clutch_batch_policy_identity::batch_policy_digest(
                &clutch_batch_policy_identity::general_clearing_v1::GENERAL_CLEARING_POLICY_V1,
            )
            .expect("the frozen general-clearing policy digests")
            .0,
        );
        let batch_policy = derive(
            &pid,
            &[
                seeds::SEED_BATCH_POLICY.to_vec(),
                epoch_seed,
                policy_digest.bytes().to_vec(),
            ],
        );

        /* The person is the fee payer, and that is a size decision as much as a
         * narrative one: an eight-outcome `CreateMarket` carries twenty-six
         * accounts, and a second writable signer -- one more 64-byte signature
         * and one more 32-byte key -- puts the message 52 bytes past the
         * 1232-byte legacy packet limit.  Folding the creator into the payer
         * removes exactly that, and it reads correctly too: at this bench you
         * are the operator, so you pay the fees and you found the market. */
        let mut actors = Vec::new();
        for (role, label, key, token) in [
            (
                "human",
                "you (also the fee payer)",
                shared.payer.bytes,
                shared.payer_collateral_token.clone(),
            ),
            (
                "bot",
                "fixed-belief automaton",
                shared.holder.bytes,
                shared.holder_collateral_token.clone(),
            ),
        ] {
            let id = Hash32::from_bytes(key);
            let position = derive(
                &pid,
                &[
                    seeds::SEED_POSITION.to_vec(),
                    market_seed.clone(),
                    id.bytes().to_vec(),
                ],
            );
            let replay = derive(
                &pid,
                &[
                    seeds::SEED_REPLAY.to_vec(),
                    market_seed.clone(),
                    id.bytes().to_vec(),
                    0_u64.to_le_bytes().to_vec(),
                ],
            );
            actors.push(Actor {
                role,
                label,
                key,
                id,
                position,
                replay,
                token,
            });
        }

        Self {
            shared,
            market_id,
            epoch_id,
            policy_digest,
            terms_value,
            grid_value,
            terms,
            grid,
            batch_policy,
            market,
            hoard,
            kernel,
            supply,
            resolution,
            hoard_authority,
            hoard_token,
            outcome_mints,
            epoch,
            window,
            page,
            pot,
            actors,
        }
    }

    pub fn actor(&self, role: &str) -> Option<&Actor> {
        self.actors.iter().find(|actor| actor.role == role)
    }

    /// One reservation address, for an order this session is about to place.
    pub fn reservation(&self, owner: Hash32, order_id: Hash32) -> Pda {
        let id = clutch_solana_layout::reservation::canonical_reservation_id(
            self.market_id,
            self.epoch_id,
            owner,
            0,
            order_id,
        );
        derive(
            &self.shared.program.address,
            &[seeds::SEED_RESERVATION.to_vec(), id.bytes().to_vec()],
        )
    }

    pub fn candidate_record(&self, candidate: Hash32) -> Pda {
        derive(
            &self.shared.program.address,
            &[
                seeds::SEED_CANDIDATE.to_vec(),
                self.epoch_id.bytes().to_vec(),
                candidate.bytes().to_vec(),
            ],
        )
    }

    pub fn candidate_feed(&self, candidate: Hash32) -> Pda {
        derive(
            &self.shared.program.address,
            &[
                seeds::SEED_CANDIDATE_FEED.to_vec(),
                self.epoch_id.bytes().to_vec(),
                candidate.bytes().to_vec(),
            ],
        )
    }

    pub fn clear_work(&self, candidate: Hash32) -> Pda {
        derive(
            &self.shared.program.address,
            &[
                seeds::SEED_CLEAR_WORK.to_vec(),
                self.epoch_id.bytes().to_vec(),
                candidate.bytes().to_vec(),
            ],
        )
    }

    pub fn receipt(&self, candidate: Hash32, slice_index: u16) -> Pda {
        derive(
            &self.shared.program.address,
            &[
                seeds::SEED_RECEIPT.to_vec(),
                self.epoch_id.bytes().to_vec(),
                candidate.bytes().to_vec(),
                slice_index.to_le_bytes().to_vec(),
            ],
        )
    }

    /// The genesis rows this session installs before slot zero.
    ///
    /// Named honestly on the banner as *precreated program accounts*: nothing
    /// below is a lifecycle a permissionless caller could have driven from a
    /// blank bank, and the market itself is deliberately **not** here.
    pub fn genesis(&self) -> Vec<Row> {
        let shared = &self.shared;
        let program = shared.program.address.clone();
        let token = base58_of(&shared.token_program);
        let system = base58_of(&shared.system_program);
        let mut rows = Vec::new();
        let mut owned = |role: &str, pda: &Pda, owner: &str, data: Vec<u8>| {
            rows.push(Row {
                role: role.to_string(),
                address: pda.address.clone(),
                owner: owner.to_string(),
                data,
            });
        };
        owned(
            "realm",
            &shared.realm,
            &program,
            shared.realm_bytes.to_vec(),
        );
        owned(
            "profile",
            &shared.profile,
            &program,
            shared.profile_bytes.to_vec(),
        );
        owned(
            "collateral-policy",
            &shared.policy_account,
            &program,
            shared.policy_bytes.to_vec(),
        );
        owned(
            "friday-grid",
            &self.grid,
            &program,
            encode::<{ account_len::PRICE_GRID }, _>(|out| self.grid_value.encode(out)),
        );
        owned(
            "friday-terms",
            &self.terms,
            &program,
            encode::<{ account_len::TERMS }, _>(|out| self.terms_value.encode(out)),
        );
        owned(
            "batch-policy",
            &self.batch_policy,
            &program,
            clutch_batch_policy_identity::canonical_batch_policy_bytes(
                &clutch_batch_policy_identity::general_clearing_v1::GENERAL_CLEARING_POLICY_V1,
            )
            .expect("the frozen policy encodes")
            .to_vec(),
        );
        owned(
            "mock-source-spec",
            &shared.source_spec,
            &program,
            shared.source_spec_bytes.clone(),
        );
        owned(
            "collateral-mint",
            &shared.collateral_mint,
            &token,
            shared.collateral_mint_bytes.clone(),
        );
        for actor in &self.actors {
            owned(
                &format!("{}-collateral", actor.role),
                &actor.token,
                &token,
                clutch_sbf_harness::token_account_bytes(
                    shared.collateral_mint.bytes,
                    actor.key,
                    WALLET_ATOMS,
                ),
            );
            /* The fee payer already has lamports: `solana-test-validator
             * --mint` funds it at genesis, and a `--account` row at the same
             * address would be a second answer to who it is. */
            if actor.key != shared.payer.bytes {
                owned(
                    &format!("{}-lamports", actor.role),
                    &Pda {
                        address: base58_of(&actor.key),
                        bytes: actor.key,
                        bump: 0,
                    },
                    &system,
                    Vec::new(),
                );
            }
        }
        rows
    }
}
