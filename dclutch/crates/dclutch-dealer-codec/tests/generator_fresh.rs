//! Exact freshness check for the checked-in Lean-generated module.

use std::path::PathBuf;
use std::process::Command;

/// The emitter's stdout, formatted the way this tree's one formatting authority
/// formats it (`rustfmt.toml` sets `style_edition`, so `cargo fmt`,
/// `tools/lane.sh fmt` and a bare `rustfmt` all produce the same bytes).
///
/// Every test below used to compare RAW emitter stdout. That holds
/// `committed == emission`, which is green only while the emission is already a
/// rustfmt fixpoint and reds the first time anyone runs `tools/lane.sh fmt` on a
/// `do not edit` file -- a direct rustfmt never sees the `#[rustfmt::skip]` that
/// lives in `src/lib.rs`, which is exactly how
/// `generated_transition_programs_v3.rs` went red at `ea4c46e02`. Two of this
/// crate's five emissions moved under rustfmt and sat in
/// `tools/emission-guard/fixpoint-debt.tsv`; the other three did not, and are
/// normalised here as well, so this guard's promise is true of every emitter it
/// re-runs rather than of two of them -- the census reads `normalises` per
/// GUARD and exempts every emitter the guard covers.
///
/// `tag` is per emitter on purpose. `cargo test` runs a binary's tests
/// concurrently, so a temporary named only by process id would be five tests
/// writing and deleting one path.
fn rustfmt_normalised(stdout: &[u8], tag: &str) -> Vec<u8> {
    let temporary = std::env::temp_dir().join(format!("dclutch-{tag}-{}.rs", std::process::id()));
    std::fs::write(&temporary, stdout).expect("write generated Rust");
    let formatted = Command::new("rustfmt")
        .args(["--edition", "2024"])
        .arg(&temporary)
        .output()
        .expect("launch rustfmt");
    assert!(
        formatted.status.success(),
        "rustfmt failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&formatted.stdout),
        String::from_utf8_lossy(&formatted.stderr)
    );
    let normalised = std::fs::read(&temporary).expect("read formatted generated Rust");
    std::fs::remove_file(&temporary).expect("remove generated Rust");
    normalised
}

#[test]
fn checked_in_rust_is_exact_lean_generator_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .args(["build", "DClutchSemantics.DealerLiquidityAbi"])
        .current_dir(&formal)
        .output()
        .expect("build imported Lean semantic library");
    assert!(
        build.status.success(),
        "semantic library build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = Command::new("lake")
        .args(["env", "lean", "--run", "EmitDealerLiquidityAbiRust.lean"])
        .current_dir(&formal)
        .output()
        .expect("run Lean Dealer ABI generator");
    assert!(
        output.status.success(),
        "generator failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let checked_in = std::fs::read(manifest.join("src/generated_dealer_liquidity.rs"))
        .expect("read checked-in generated Dealer module");
    let formatted = rustfmt_normalised(&output.stdout, "dealer-liquidity");
    assert_eq!(formatted, checked_in, "regenerate the Dealer ABI module");
}

#[test]
fn checked_in_trading_tail_is_exact_lean_generator_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .args(["build", "DClutchSemantics.DealerTradingProfile"])
        .current_dir(&formal)
        .output()
        .expect("build Dealer Trading profile");
    assert!(
        build.status.success(),
        "Dealer Trading profile build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = Command::new("lake")
        .args(["env", "lean", "--run", "EmitDealerTradingProfileRust.lean"])
        .current_dir(&formal)
        .output()
        .expect("run Dealer Trading profile generator");
    assert!(
        output.status.success(),
        "Dealer Trading profile generator failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let checked_in = std::fs::read(manifest.join("src/generated_dealer_trading_profile.rs"))
        .expect("read checked-in generated Dealer Trading profile");
    let formatted = rustfmt_normalised(&output.stdout, "dealer-trading-profile");
    assert_eq!(
        formatted, checked_in,
        "regenerate the Dealer Trading profile ABI module"
    );
}

#[test]
fn checked_in_scenario_trade_header_is_exact_lean_generator_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .args(["build", "DClutchSemantics.DealerScenarioTradeV4Abi"])
        .current_dir(&formal)
        .output()
        .expect("build Dealer scenario trade header ABI");
    assert!(
        build.status.success(),
        "scenario trade header ABI build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = Command::new("lake")
        .args(["env", "lean", "--run", "EmitDealerScenarioTradeV4Rust.lean"])
        .current_dir(&formal)
        .output()
        .expect("run Dealer scenario trade header generator");
    assert!(
        output.status.success(),
        "scenario trade header generator failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let checked_in = std::fs::read(manifest.join("src/generated_scenario_trade_v4.rs"))
        .expect("read checked-in generated scenario trade header");
    let formatted = rustfmt_normalised(&output.stdout, "dealer-scenario-trade-v4");
    assert_eq!(
        formatted, checked_in,
        "regenerate the Dealer scenario trade header ABI module"
    );
}

/// The Dealer scenario checkpoint's layout and tag ABI.
///
/// One of the four machines the route census gates on that had no Lean owner
/// at all: five discriminants and thirty-one coordinates authored by
/// `scenario_checkpoint_v1.rs` alone, with nothing in the repository re-running
/// anything. This used to say it was "a raw byte compare rather than a
/// rustfmt-normalized one because the emitter's own output is already
/// rustfmt-stable". That is true of THIS emission and was not true of the two
/// this file also guards, and a guard normalises for all of its emitters or
/// none, so it normalises here too -- and a property that happens to hold is
/// not a property a comparison should depend on.
#[test]
fn checked_in_scenario_checkpoint_is_exact_lean_generator_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .args(["build", "DClutchSemantics.DealerScenarioCheckpointV1Abi"])
        .current_dir(&formal)
        .output()
        .expect("build Dealer scenario checkpoint ABI");
    assert!(
        build.status.success(),
        "Dealer scenario checkpoint ABI build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = Command::new("lake")
        .args([
            "env",
            "lean",
            "--run",
            "EmitDealerScenarioCheckpointV1Rust.lean",
        ])
        .current_dir(&formal)
        .output()
        .expect("run Dealer scenario checkpoint generator");
    assert!(
        output.status.success(),
        "Dealer scenario checkpoint generator failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let emitted = String::from_utf8(output.stdout).expect("generator emitted UTF-8");
    // Pinned before the compare, because a tag that silently moved would let a
    // client call a rolling-back checkpoint committable, and a moved digest run
    // would make every checkpoint opened before it undecodable.
    for pin in [
        "pub const DEALER_SCENARIO_CHECKPOINT_BYTES_V1: usize = 944;",
        "pub const DEALER_SCENARIO_CHECKPOINT_PHASE_OFFSET_V1: usize = 10;",
        "pub const DEALER_SCENARIO_CHECKPOINT_PHASE_COLLECTING_V1: u8 = 1;",
        "pub const DEALER_SCENARIO_CHECKPOINT_PHASE_EVALUATED_V1: u8 = 2;",
        "pub const DEALER_SCENARIO_CHECKPOINT_PHASE_RESERVED_V1: u8 = 3;",
        "pub const DEALER_SCENARIO_CHECKPOINT_PHASE_ROLLING_BACK_V1: u8 = 4;",
        "pub const DEALER_SCENARIO_CHECKPOINT_PHASE_COMMITTED_V1: u8 = 5;",
        "pub const DEALER_SCENARIO_PREPARATION_PAGES_V1: usize = 6;",
        "pub const DEALER_SCENARIO_CHECKPOINT_PAGE_RECEIPT_DIGESTS_OFFSET_V1: usize = 400;",
        "pub const DEALER_SCENARIO_CHECKPOINT_RESERVATION_RECEIPT_DIGESTS_OFFSET_V1: usize = 816;",
    ] {
        assert!(
            emitted.lines().any(|line| line == pin),
            "the emitted checkpoint ABI no longer states `{pin}`"
        );
    }
    let checked_in =
        std::fs::read_to_string(manifest.join("src/generated_scenario_checkpoint_v1.rs"))
            .expect("read checked-in generated Dealer scenario checkpoint module");
    let formatted = String::from_utf8(rustfmt_normalised(
        emitted.as_bytes(),
        "dealer-scenario-checkpoint",
    ))
    .expect("rustfmt emitted UTF-8");
    assert_eq!(
        formatted, checked_in,
        "regenerate the Dealer scenario checkpoint ABI module"
    );
}

/// The Dealer scenario reservation state's layout and tag ABI.
///
/// The last of the four machines the route census gates on that had no Lean
/// owner at all. Three discriminants and twenty-five coordinates authored by
/// `scenario_custody_reservation_v1.rs` alone, in a file where four records
/// share one header shape through four constants none of them owns.
#[test]
fn checked_in_scenario_reservation_state_is_exact_lean_generator_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .args([
            "build",
            "DClutchSemantics.DealerScenarioReservationStateV1Abi",
        ])
        .current_dir(&formal)
        .output()
        .expect("build Dealer scenario reservation state ABI");
    assert!(
        build.status.success(),
        "Dealer scenario reservation state ABI build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = Command::new("lake")
        .args([
            "env",
            "lean",
            "--run",
            "EmitDealerScenarioReservationStateV1Rust.lean",
        ])
        .current_dir(&formal)
        .output()
        .expect("run Dealer scenario reservation state generator");
    assert!(
        output.status.success(),
        "Dealer scenario reservation state generator failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let emitted = String::from_utf8(output.stdout).expect("generator emitted UTF-8");
    // Pinned before the compare: a status tag that silently moved would let a
    // client call an escrowed reservation delivered, and a moved reserved span
    // would make every reservation opened before it non-canonical.
    for pin in [
        "pub const DEALER_SCENARIO_RESERVATION_STATE_BYTES_V1: usize = 512;",
        "pub const DEALER_SCENARIO_RESERVATION_STATE_STATUS_OFFSET_V1: usize = 10;",
        "pub const DEALER_SCENARIO_RESERVATION_STATUS_ACTIVE_V1: u8 = 1;",
        "pub const DEALER_SCENARIO_RESERVATION_STATUS_ROLLED_BACK_V1: u8 = 2;",
        "pub const DEALER_SCENARIO_RESERVATION_STATUS_ACTIVATED_V1: u8 = 3;",
        "pub const DEALER_SCENARIO_RESERVATION_STATE_HEAD_RESERVED_OFFSET_V1: usize = 13;",
        "pub const DEALER_SCENARIO_RESERVATION_STATE_RESERVED_OFFSET_V1: usize = 496;",
        "pub const DEALER_SCENARIO_RESERVATION_STATE_AMOUNT_OFFSET_V1: usize = 464;",
        // The two words all four records in `scenario_custody_reservation_v1.rs`
        // share, which `5f8a09971` left as a `const _: () = assert!` because no
        // Lean module owned them. `require_header` and `put_header` read these
        // for the custody effect, the effect manifest, the reservation batch and
        // this state alike, so a header that moved would make all four
        // unreadable at once.
        "pub const DEALER_SCENARIO_CUSTODY_HEADER_MAGIC_OFFSET_V1: usize = 0;",
        "pub const DEALER_SCENARIO_CUSTODY_HEADER_MAGIC_BYTES_V1: usize = 8;",
        "pub const DEALER_SCENARIO_CUSTODY_HEADER_VERSION_OFFSET_V1: usize = 8;",
        "pub const DEALER_SCENARIO_CUSTODY_HEADER_VERSION_BYTES_V1: usize = 2;",
        "pub const DEALER_SCENARIO_CUSTODY_HEADER_BYTES_V1: usize = 10;",
    ] {
        assert!(
            emitted.lines().any(|line| line == pin),
            "the emitted reservation state ABI no longer states `{pin}`"
        );
    }
    let checked_in =
        std::fs::read_to_string(manifest.join("src/generated_scenario_reservation_state_v1.rs"))
            .expect("read checked-in generated Dealer scenario reservation state module");
    let formatted = String::from_utf8(rustfmt_normalised(
        emitted.as_bytes(),
        "dealer-scenario-reservation-state",
    ))
    .expect("rustfmt emitted UTF-8");
    assert_eq!(
        formatted, checked_in,
        "regenerate the Dealer scenario reservation state ABI module"
    );
}
