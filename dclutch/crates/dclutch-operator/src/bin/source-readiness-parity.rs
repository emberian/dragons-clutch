//! Native parity oracle for the generated Source-readiness WASM artifact.

use std::io::{self, Read};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use dclutch_market::{CoreState, Identity, MarketIdentity, Phase, Readiness, StateBumpsV1};
use dclutch_operator::source_readiness::{
    derive_source_close_detail_json_v1, derive_source_readiness_base_json_v1,
    derive_source_terminal_base_json_v1,
};
use serde_json::json;
use solana_program::pubkey::Pubkey;

fn identity(value: u8) -> Identity {
    Identity::new([value; 32]).expect("nonzero parity identity")
}

fn key(value: u8) -> Pubkey {
    Pubkey::new_from_array([value; 32])
}

fn fixture(format: &str) -> String {
    let market_key = key(1);
    let core_program = key(9);
    let registry_program = key(8);
    let resolution_program = key(10);
    let market = CoreState {
        phase: Phase::Founding,
        readiness: Readiness::Prepaid,
        terminal_winner: 0,
        identity: MarketIdentity {
            market_id: Identity::new(market_key.to_bytes()).expect("Market"),
            realm_id: identity(2),
            product_record: identity(3),
            product_id: identity(4),
            resolution_policy: identity(5),
            capability_manifest: identity(6),
            selected_release_set: identity(7),
            registry_program: Identity::new(registry_program.to_bytes()).expect("Registry"),
            generation: 11,
        },
        outstanding_capabilities: 0,
        principal_cap_sets: u64::MAX,
        rent_beneficiary: identity(12),
        terminal_receipt: None,
        bumps: StateBumpsV1::UNRECORDED,
    }
    .encode()
    .expect("canonical parity Market");
    json!({
        "format": format,
        "market": {
            "address": market_key.to_string(),
            "owner": core_program.to_string(),
            "executable": false,
            "dataBase64": STANDARD.encode(market),
        },
        "coreProgram": core_program.to_string(),
        "registryProgram": registry_program.to_string(),
        "resolutionProgram": resolution_program.to_string(),
    })
    .to_string()
}

fn close_fixture() -> String {
    concat!(
        "{\"format\":\"dclutch-source-close-detail-v1\",",
        "\"marketAddress\":\"4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi\",",
        "\"marketDataBase64\":\"RENMVENPUjMDAAMCAAAAAAEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgIDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHCAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgLAAAAAAAAAAAAAAAAAAAA//////////8MDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDA0NDQ0NDQ0NDQ0NDQ0NDQ0NDQ0NDQ0NDQ0NDQ0NDQ0NAAAAAAAAAAA=\",",
        "\"resolutionProgram\":\"gBxS1f6uyyGPuW5MzGBukidSb71jdsCb5fZaoSzULE5\",",
        "\"sourceStateAddress\":\"Bt11rGReD84MwjjiYbdygw6AQoCHKm8Xynw7z5GuERzx\",",
        "\"sourceStateDataBase64\":\"RENMVFNSUzICAAIAAQEAAAAAAAAAAAAAAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQELAAAAAAAAAAUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABsbGxsbGxsbGxsbGxsbGxsbGxsbGxsbGxsbGxsbGxsbCQAAAAAAAABkAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\"}"
    )
    .to_owned()
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    if mode == "fixture-base" {
        print!("{}", fixture("dclutch-source-readiness-market-v1"));
        return;
    }
    if mode == "fixture-terminal-base" {
        print!("{}", fixture("dclutch-source-terminal-base-v1"));
        return;
    }
    if mode == "fixture-close-detail" {
        print!("{}", close_fixture());
        return;
    }
    if mode != "base" && mode != "terminal-base" && mode != "close-detail" {
        eprintln!("expected a base fixture or native base mode");
        std::process::exit(2);
    }
    let mut source = Vec::new();
    io::stdin()
        .read_to_end(&mut source)
        .expect("read parity input");
    let result = if mode == "base" {
        derive_source_readiness_base_json_v1(&source)
    } else if mode == "terminal-base" {
        derive_source_terminal_base_json_v1(&source)
    } else {
        derive_source_close_detail_json_v1(&source)
    };
    match result {
        Ok(output) => print!("{output}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
