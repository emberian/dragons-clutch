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
    let checked_in = std::fs::read(manifest.join("src/dealer/generated_dealer_liquidity.rs"))
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
    let checked_in = std::fs::read(manifest.join("src/dealer/generated_dealer_trading_profile.rs"))
        .expect("read checked-in generated Dealer Trading profile");
    let formatted = rustfmt_normalised(&output.stdout, "dealer-trading-profile");
    assert_eq!(
        formatted, checked_in,
        "regenerate the Dealer Trading profile ABI module"
    );
}
