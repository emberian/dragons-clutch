//! Seed construction for the Direct V2 interpreted-vs-AOT CU measurement.
//!
//! The seeds are deterministic and shared by both ELFs: the same 32 register
//! banks are submitted to the interpreted twin and the AOT twin, so a CU
//! difference cannot be an artifact of different inputs. Seed 0 is the
//! canonical formal example; the rest perturb the fields the relation actually
//! branches on, and deliberately include semantic refusals as well as
//! acceptances, because a route pays for refusals too.

use dclutch_core_contract::ContentId;
use dclutch_direct_aot_contract::*;
use dclutch_execution_strategy_contract::{AcceleratorRequestV1, encode_register_bank_into};

/// Exact Direct scalar-bank width.
pub const SCALARS: usize = DIRECT_PROGRAM_V2_SCALARS as usize;
/// Exact Direct identity-bank width.
pub const IDENTITIES: usize = DIRECT_PROGRAM_V2_IDENTITIES as usize;
/// Exact Direct scalar-then-identity bank bytes.
pub const BANK_BYTES: usize = 456;
/// Exact Direct accelerator request bytes.
pub const REQUEST_BYTES: usize = 584;
/// The measurement seed count. Thirty-two, never twelve.
pub const SEED_COUNT: usize = 32;

/// Deterministic splitmix64, so a reported number can be reproduced exactly.
#[derive(Clone, Copy, Debug)]
pub struct Rng(u64);

impl Rng {
    /// Seed the generator.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1))
    }

    /// Next value.
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Next value in `[low, high]`.
    pub fn range(&mut self, low: u64, high: u64) -> u64 {
        if high <= low {
            return low;
        }
        low + (self.next() % (high - low + 1))
    }
}

fn content(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).expect("content")
}

/// The canonical formal example: the bank the AOT crate's own equivalence test
/// uses, which the shipped accelerator ELF accepts.
#[must_use]
pub fn example_scalars() -> [u64; SCALARS] {
    let mut scalars = [0_u64; SCALARS];
    scalars[SCALAR_PHASE] = OPEN_PHASE_V2;
    scalars[SCALAR_SLOT] = 100;
    scalars[SCALAR_SELLER_FROM] = 90;
    scalars[SCALAR_SELLER_THROUGH] = 110;
    scalars[SCALAR_BUYER_FROM] = 95;
    scalars[SCALAR_BUYER_THROUGH] = 120;
    scalars[SCALAR_SELLER_SIDE] = SELL_SIDE_V2;
    scalars[SCALAR_BUYER_SIDE] = BUY_SIDE_V2;
    scalars[SCALAR_SELLER_GENERATION] = 3;
    scalars[SCALAR_BUYER_GENERATION] = 3;
    scalars[SCALAR_SELLER_OUTCOME] = 1;
    scalars[SCALAR_BUYER_OUTCOME] = 1;
    scalars[SCALAR_OUTCOME_COUNT] = 2;
    scalars[SCALAR_SELLER_LIFECYCLE] = 0;
    scalars[SCALAR_SELLER_MAXIMUM] = 2_000;
    scalars[SCALAR_BUYER_LIFECYCLE] = 0;
    scalars[SCALAR_BUYER_MAXIMUM] = 2_000;
    scalars[SCALAR_SELLER_LIMIT] = 400_000;
    scalars[SCALAR_EXECUTION_PRICE] = 500_000;
    scalars[SCALAR_BUYER_LIMIT] = 600_000;
    scalars[SCALAR_PRICE_SCALE] = 1_000_000;
    scalars[SCALAR_SELLER_FEE_BPS] = 25;
    scalars[SCALAR_BUYER_FEE_BPS] = 25;
    scalars[SCALAR_POLICY_FEE_BPS] = 25;
    scalars[SCALAR_FILL] = 2_000;
    scalars[SCALAR_SELLER_CLAIMS] = 5_000;
    scalars[SCALAR_BUYER_CLAIMS] = 200;
    scalars[SCALAR_BUYER_COLLATERAL] = 2_000;
    scalars[SCALAR_SELLER_COLLATERAL] = 100;
    scalars[SCALAR_VENUE_COLLATERAL] = 20;
    scalars
}

/// The first seed index that carries a designed refusal.
pub const FIRST_REFUSAL_SEED: usize = 24;

/// Build an admissible bank: every conjunct of the relation is satisfied by
/// construction, so the seed exercises the whole program to its last check.
///
/// Constructing rather than sampling is deliberate. The relation is a long
/// conjunction including `fill == maximum` under FOK, `nonce == next_nonce`,
/// three equal fee rates, and an *exact* `fill * price / scale`; independent
/// random draws refuse almost immediately and would have priced the cheap
/// early-exit path while claiming to price the route.
fn admissible(index: usize) -> [u64; SCALARS] {
    let mut scalars = example_scalars();
    let mut rng = Rng::new(index as u64);

    let scale = 1_000_000_u64;
    scalars[SCALAR_PRICE_SCALE] = scale;

    // Keep price and fill on thousand-boundaries so `fill * price` is an exact
    // multiple of the scale and the division cannot refuse.
    let execution = 1_000 * rng.range(1, 1_000);
    scalars[SCALAR_EXECUTION_PRICE] = execution;
    scalars[SCALAR_SELLER_LIMIT] = execution.saturating_sub(1_000 * rng.range(0, 200));
    scalars[SCALAR_BUYER_LIMIT] = execution.saturating_add(1_000 * rng.range(0, 200));

    let fill = 1_000 * rng.range(1, 200);
    scalars[SCALAR_FILL] = fill;

    // FOK demands an exact fill; IOC and GTC admit a maximum at or above it.
    let lifecycle = rng.range(0, 2);
    let maximum = if lifecycle == 0 {
        fill
    } else {
        fill.saturating_add(1_000 * rng.range(0, 100))
    };
    scalars[SCALAR_SELLER_LIFECYCLE] = lifecycle;
    scalars[SCALAR_BUYER_LIFECYCLE] = lifecycle;
    scalars[SCALAR_SELLER_MAXIMUM] = maximum;
    scalars[SCALAR_BUYER_MAXIMUM] = maximum;

    // One policy rate, mirrored by both sides, as the relation requires.
    let fee_bps = rng.range(0, 500);
    scalars[SCALAR_SELLER_FEE_BPS] = fee_bps;
    scalars[SCALAR_BUYER_FEE_BPS] = fee_bps;
    scalars[SCALAR_POLICY_FEE_BPS] = fee_bps;

    let slot = rng.range(50, 200);
    scalars[SCALAR_SLOT] = slot;
    scalars[SCALAR_SELLER_FROM] = slot.saturating_sub(rng.range(0, 20));
    scalars[SCALAR_SELLER_THROUGH] = slot.saturating_add(rng.range(0, 20));
    scalars[SCALAR_BUYER_FROM] = slot.saturating_sub(rng.range(0, 20));
    scalars[SCALAR_BUYER_THROUGH] = slot.saturating_add(rng.range(0, 20));

    let generation = rng.range(0, 8);
    scalars[SCALAR_SELLER_GENERATION] = generation;
    scalars[SCALAR_BUYER_GENERATION] = generation;

    let outcome_count = rng.range(2, 8);
    scalars[SCALAR_OUTCOME_COUNT] = outcome_count;
    let outcome = rng.range(0, outcome_count - 1);
    scalars[SCALAR_SELLER_OUTCOME] = outcome;
    scalars[SCALAR_BUYER_OUTCOME] = outcome;

    let nonce = rng.range(0, 1_000);
    scalars[SCALAR_SELLER_NONCE] = nonce;
    scalars[SCALAR_SELLER_NEXT_NONCE] = nonce;
    scalars[SCALAR_BUYER_NONCE] = nonce;
    scalars[SCALAR_BUYER_NEXT_NONCE] = nonce;

    // Balances that comfortably cover the computed gross and fee.
    let gross = fill.saturating_mul(execution) / scale;
    let fee = gross.saturating_mul(fee_bps) / 10_000;
    scalars[SCALAR_SELLER_CLAIMS] = fill.saturating_add(rng.range(0, 100_000));
    scalars[SCALAR_BUYER_CLAIMS] = rng.range(0, 100_000);
    scalars[SCALAR_BUYER_COLLATERAL] = gross
        .saturating_add(fee)
        .saturating_add(rng.range(0, 100_000));
    scalars[SCALAR_SELLER_COLLATERAL] = rng.range(0, 100_000);
    scalars[SCALAR_VENUE_COLLATERAL] = rng.range(0, 100_000);

    scalars
}

/// Build seed `index`'s scalar bank.
///
/// Seed 0 is the unperturbed formal example. Seeds 1..24 are admissible by
/// construction. Seeds 24..32 each take an admissible bank and violate exactly
/// one conjunct, chosen to land at a different depth in the program — the
/// earliest check, the middle, and the very last balance test. Refusal depth is
/// precisely where an interpreter is expected to lose most to compiled code, so
/// pricing only acceptances, or only shallow refusals, would flatter one side.
#[must_use]
pub fn seed_scalars(index: usize) -> [u64; SCALARS] {
    if index == 0 {
        return example_scalars();
    }
    let mut scalars = admissible(index);
    if index < FIRST_REFUSAL_SEED {
        return scalars;
    }
    match index - FIRST_REFUSAL_SEED {
        // Earliest conjunct: the Market is not open.
        0 => scalars[SCALAR_PHASE] = 0,
        // Second conjunct: a zero fill.
        1 => scalars[SCALAR_FILL] = 0,
        // Early: the slot sits outside the seller's window.
        2 => scalars[SCALAR_SELLER_THROUGH] = scalars[SCALAR_SLOT].saturating_sub(1),
        // Middle: the two sides disagree about the Market generation.
        3 => {
            scalars[SCALAR_SELLER_GENERATION] = scalars[SCALAR_BUYER_GENERATION].saturating_add(1);
        }
        // Middle: an unknown lifecycle tag, the one non-CheckFailed refusal.
        4 => scalars[SCALAR_SELLER_LIFECYCLE] = 7,
        // Late: the price no longer divides the fill exactly. The buyer limit
        // moves with it so the refusal lands on the division, not on the
        // earlier limit ordering.
        5 => {
            scalars[SCALAR_EXECUTION_PRICE] = scalars[SCALAR_EXECUTION_PRICE].saturating_add(1);
            scalars[SCALAR_BUYER_LIMIT] = scalars[SCALAR_BUYER_LIMIT].saturating_add(1_000);
        }
        // Late: the seller does not hold enough claims.
        6 => scalars[SCALAR_SELLER_CLAIMS] = scalars[SCALAR_FILL].saturating_sub(1),
        // Last conjunct reachable: the buyer cannot cover gross plus fee.
        _ => scalars[SCALAR_BUYER_COLLATERAL] = 0,
    }
    scalars
}

/// Encode seed `index` as the exact 584-byte accelerator request wire.
#[must_use]
pub fn seed_request(index: usize) -> [u8; REQUEST_BYTES] {
    let scalars = seed_scalars(index);
    let identities = [[101_u8; 32], [101_u8; 32], [11_u8; 32], [12_u8; 32]];
    let mut bank = [0_u8; BANK_BYTES];
    encode_register_bank_into(&scalars, &identities, &mut bank).expect("register bank");
    let request = AcceleratorRequestV1::new(
        content(1),
        content(2),
        content(3),
        DIRECT_PROGRAM_V2_SCALARS,
        DIRECT_PROGRAM_V2_IDENTITIES,
        &bank,
    )
    .expect("request");
    let mut bytes = [0_u8; REQUEST_BYTES];
    request.encode_into(&mut bytes).expect("request bytes");
    bytes
}
