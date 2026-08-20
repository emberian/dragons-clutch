// SPDX-License-Identifier: AGPL-3.0-or-later
//! Print exact account widths and pinned `Rent::default()` minima.
//!
//! This probe emits facts only.  It deliberately does not construct a global
//! `LivenessPolicy`: the measured profile remains STOP while any mandatory
//! route, source release, storage owner, or terminal path is unadmitted.

use clutch_batch_policy_identity::{
    direct_window_v1::{DIRECT_CANDIDATE_ACCOUNT_BYTES, DIRECT_WINDOW_ACCOUNT_BYTES},
    BATCH_POLICY_BYTES,
};
use clutch_solana_layout::{
    account_len,
    artifact::{ARTIFACT_STAGE_HEADER_BYTES, MAX_ARTIFACT_BYTES},
    clearing::EPOCH_WINDOW_ACCOUNT_BYTES,
    collateral::COLLATERAL_POLICY_BYTES,
    direct_selection::DIRECT_EPOCH_BYTES,
    native_resolution::NATIVE_RESOLUTION_LEN,
    occupation_resolution::OCCUPATION_RESOLUTION_LEN,
    reservation::RESERVATION_ACCOUNT_BYTES,
    resolution_work::RESOLUTION_WORK_ACCOUNT_BYTES,
};
use clutch_solana_reference::{KERNEL_ACCOUNT_LEN, REPLAY_ACCOUNT_LEN};
use solana_rent::{Rent, ACCOUNT_STORAGE_OVERHEAD, DEFAULT_LAMPORTS_PER_BYTE};

// These widths are owned by the SBF adapter or native programs rather than the layout
// crates. The Python checker pins them against compile-time assertions in the
// named source files before accepting this probe's output.
const SOURCE_SPEC_BYTES: usize = 292;
const SOURCE_ARCHIVE_BYTES: usize = 2_560;
const TOKEN_MINT_BYTES: usize = 82;
const IMMUTABLE_OWNER_TOKEN_BYTES: usize = 170;

fn row(name: &str, bytes: usize, rent: &Rent) {
    println!("{name}\t{bytes}\t{}", rent.minimum_balance(bytes).max(1));
}

fn main() {
    let rent = Rent::default();
    println!("schema\tdragons-clutch/liveness-account-inventory/v2");
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
    row("artifact.batch_policy.final", BATCH_POLICY_BYTES, &rent);
    row(
        "artifact.batch_policy.stage",
        ARTIFACT_STAGE_HEADER_BYTES + BATCH_POLICY_BYTES,
        &rent,
    );

    row("realm", account_len::REALM, &rent);
    row("profile", account_len::PROFILE, &rent);
    row("market", account_len::MARKET, &rent);
    row("hoard", account_len::HOARD, &rent);
    row("position", account_len::POSITION, &rent);
    row("feed", account_len::FEED, &rent);
    row("kernel", KERNEL_ACCOUNT_LEN, &rent);
    row("replay", REPLAY_ACCOUNT_LEN, &rent);
    row("supply_ledger", account_len::SUPPLY_LEDGER, &rent);
    row("resolution.v2", account_len::RESOLUTION, &rent);
    row("resolution.v3", NATIVE_RESOLUTION_LEN, &rent);
    row("resolution.v4", OCCUPATION_RESOLUTION_LEN, &rent);
    row("token.outcome_mint", TOKEN_MINT_BYTES, &rent);
    row(
        "token.hoard_immutable_owner",
        IMMUTABLE_OWNER_TOKEN_BYTES,
        &rent,
    );

    row("order.page", account_len::ORDER_PAGE, &rent);
    row("order.reservation.v1", RESERVATION_ACCOUNT_BYTES, &rent);
    // The general epoch plane (T2-6) reuses the epoch/candidate/feed/
    // clear-work account families below at their exact widths; its one new
    // persistent family is the deadline-window companion created by
    // InitEpoch (tag 49) at seeds::epoch_window_pda.  No handler closes it.
    row("epoch.window", EPOCH_WINDOW_ACCOUNT_BYTES, &rent);
    row("legacy.epoch.v2", account_len::EPOCH, &rent);
    row("legacy.candidate", account_len::CANDIDATE, &rent);
    row("legacy.candidate_feed", account_len::CANDIDATE_FEED, &rent);
    row("legacy.clear_work", account_len::CLEAR_WORK, &rent);
    row("direct.epoch.v3", DIRECT_EPOCH_BYTES, &rent);
    row("direct.candidate.v2", DIRECT_CANDIDATE_ACCOUNT_BYTES, &rent);
    row("direct.window.v1", DIRECT_WINDOW_ACCOUNT_BYTES, &rent);
    row("direct.receipt", account_len::SETTLEMENT_RECEIPT, &rent);
    row("direct.final_pot", account_len::FINAL_POT, &rent);

    row("source.spec", SOURCE_SPEC_BYTES, &rent);
    row("source.archive", SOURCE_ARCHIVE_BYTES, &rent);
    row("resolution.work.v1", RESOLUTION_WORK_ACCOUNT_BYTES, &rent);
    row("resolution.reserve.v1", 0, &rent);
}
