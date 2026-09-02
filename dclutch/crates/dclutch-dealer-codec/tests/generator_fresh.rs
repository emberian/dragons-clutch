//! Exact freshness check for the checked-in Lean-generated module.

use std::path::PathBuf;
use std::process::Command;

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
    assert_eq!(
        output.stdout, checked_in,
        "regenerate the Dealer ABI module"
    );
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
    assert_eq!(
        output.stdout, checked_in,
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
    assert_eq!(
        output.stdout, checked_in,
        "regenerate the Dealer scenario trade header ABI module"
    );
}
