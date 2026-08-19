// SPDX-License-Identifier: AGPL-3.0-or-later
//! Print the exact account widths and `solana-rent = 4.3.0` default minima
//! consumed by the liveness-policy evidence manifest.

use clutch_liveness::{Id, LivenessPolicy};
use clutch_solana_layout::{
    account_len,
    artifact::{ARTIFACT_STAGE_HEADER_BYTES, MAX_ARTIFACT_BYTES},
    collateral::COLLATERAL_POLICY_BYTES,
    native_resolution::NATIVE_RESOLUTION_LEN,
    reservation::RESERVATION_ACCOUNT_BYTES,
};
use clutch_solana_reference::{KERNEL_ACCOUNT_LEN, REPLAY_ACCOUNT_LEN};
use solana_rent::{Rent, ACCOUNT_STORAGE_OVERHEAD, DEFAULT_LAMPORTS_PER_BYTE};

// These four widths are owned by the SBF adapter rather than the layout
// crates. The Python checker pins them against compile-time assertions in the
// named source files before accepting this probe's output.
const SOURCE_SPEC_BYTES: usize = 292;
const SOURCE_ARCHIVE_BYTES: usize = 2_560;
const TOKEN_MINT_BYTES: usize = 82;
const IMMUTABLE_OWNER_TOKEN_BYTES: usize = 170;

// This is an arithmetic candidate derived and audited by policy.py. It is not
// a promoted Realm policy. In particular, the resolution row does not pass the
// requested 25% CU-headroom gate, per-order storage remains unrepresented, and
// no production neutral sink has been selected.
const CANDIDATE_MARKET_WORK_LAMPORTS: u64 = 8_090_000;
const CANDIDATE_MARKET_STORAGE_LAMPORTS: u64 = 78_529_680;
const CANDIDATE_RESOLUTION_LAMPORTS: u64 = 1_510_000;
const CANDIDATE_PER_ORDER_CLEAR_LAMPORTS: u64 = 755_000;
const CANDIDATE_PER_ORDER_SETTLE_LAMPORTS: u64 = 605_000;

fn row(name: &str, bytes: usize, rent: &Rent) {
    println!("{name}\t{bytes}\t{}", rent.minimum_balance(bytes).max(1));
}

fn main() {
    let rent = Rent::default();
    println!("schema\tdragons-clutch/liveness-account-inventory/v1");
    println!("lamports_per_byte\t{DEFAULT_LAMPORTS_PER_BYTE}");
    println!("account_storage_overhead\t{ACCOUNT_STORAGE_OVERHEAD}");

    row("artifact.policy.final", COLLATERAL_POLICY_BYTES, &rent);
    row(
        "artifact.policy.stage",
        ARTIFACT_STAGE_HEADER_BYTES + COLLATERAL_POLICY_BYTES,
        &rent,
    );
    row("artifact.grid.final", account_len::PRICE_GRID, &rent);
    row(
        "artifact.grid.stage",
        ARTIFACT_STAGE_HEADER_BYTES + account_len::PRICE_GRID,
        &rent,
    );
    row("artifact.terms.final", account_len::TERMS, &rent);
    row(
        "artifact.terms.stage",
        ARTIFACT_STAGE_HEADER_BYTES + account_len::TERMS,
        &rent,
    );
    row(
        "artifact.maximum.stage",
        ARTIFACT_STAGE_HEADER_BYTES + MAX_ARTIFACT_BYTES,
        &rent,
    );

    row("realm", account_len::REALM, &rent);
    row("profile", account_len::PROFILE, &rent);
    row("market", account_len::MARKET, &rent);
    row("hoard", account_len::HOARD, &rent);
    row("position", account_len::POSITION, &rent);
    row("kernel", KERNEL_ACCOUNT_LEN, &rent);
    row("replay", REPLAY_ACCOUNT_LEN, &rent);
    row("supply_ledger", account_len::SUPPLY_LEDGER, &rent);
    row("resolution.v2", account_len::RESOLUTION, &rent);
    row("resolution.v3", NATIVE_RESOLUTION_LEN, &rent);
    row("token.outcome_mint", TOKEN_MINT_BYTES, &rent);
    row(
        "token.hoard_immutable_owner",
        IMMUTABLE_OWNER_TOKEN_BYTES,
        &rent,
    );

    row("order.page", account_len::ORDER_PAGE, &rent);
    row("order.reservation", RESERVATION_ACCOUNT_BYTES, &rent);
    row("candidate", account_len::CANDIDATE, &rent);
    row("candidate.feed", account_len::CANDIDATE_FEED, &rent);
    row("settlement.receipt", account_len::SETTLEMENT_RECEIPT, &rent);

    row("source.spec", SOURCE_SPEC_BYTES, &rent);
    row("source.archive", SOURCE_ARCHIVE_BYTES, &rent);

    let candidate = LivenessPolicy {
        market_work_max_lamports: CANDIDATE_MARKET_WORK_LAMPORTS,
        market_storage_max_lamports: CANDIDATE_MARKET_STORAGE_LAMPORTS,
        resolution_max_lamports: CANDIDATE_RESOLUTION_LAMPORTS,
        per_order_clear_max_lamports: CANDIDATE_PER_ORDER_CLEAR_LAMPORTS,
        per_order_settle_max_lamports: CANDIDATE_PER_ORDER_SETTLE_LAMPORTS,
        neutral_sink: Id::from_bytes([0xfa; 32]),
    };
    let market = candidate.market_quote().expect("candidate must fit u64");
    let order = candidate.order_quote().expect("candidate must fit u64");
    println!("candidate.status\tINTERMEDIATE_ARITHMETIC_CANDIDATE_NOT_PROMOTABLE");
    println!("candidate.market.work_lamports\t{}", market.work_lamports);
    println!(
        "candidate.market.storage_lamports\t{}",
        market.storage_lamports
    );
    println!(
        "candidate.market.resolution_lamports\t{}",
        market.resolution_lamports
    );
    println!("candidate.market.total_lamports\t{}", market.total_lamports);
    println!("candidate.order.clear_lamports\t{}", order.clear_lamports);
    println!("candidate.order.settle_lamports\t{}", order.settle_lamports);
    println!("candidate.order.total_lamports\t{}", order.total_lamports);
}
