use super::*;
use dclutch_market::capability_program::hot_v3::HOT_FIXED_ACCOUNT_COUNT_V3;
use dclutch_trading::general_codec::Action;
use serde_json::{Value, json};

fn address(value: u8) -> String {
    Pubkey::new_from_array([value; 32]).to_string()
}
fn identity(value: u8) -> String {
    format!("{value:02x}").repeat(32)
}
fn route(action: Action) -> String {
    let fixed = (1..=HOT_FIXED_ACCOUNT_COUNT_V3).map(|index| {
        json!({"address": address(u8::try_from(index).expect("small fixture")), "isSigner": false, "isWritable": index == 2})
    }).collect::<Vec<_>>();
    let strategy = (40..=47)
        .map(|value| json!({"address": address(value), "isSigner": false, "isWritable": false}))
        .collect::<Vec<_>>();
    json!({
        "format": producer::ROUTE_FORMAT_V1, "action": producer::action_name_v1(action),
        "minimumFinalizedSlot": "77", "payer": address(90), "lookupTable": address(91),
        "releaseSet": identity(1), "generation": "7",
        "checkedRelease": {"tradingProgram": address(26), "tradingArtifactRelease": identity(2), "generalArtifactRelease": identity(3), "checkedManifestDigest": identity(4)},
        "artifactSelection": {"programSet": identity(5), "config": identity(6), "artifactRelease": identity(3)},
        "fixedAccounts": fixed, "strategyAccounts": strategy, "runtimeSuffixAccounts": []
    }).to_string()
}
fn input() -> Value {
    let route = route(Action::OpenBatch);
    let parsed = producer::parse_route_v1(route.as_bytes()).expect("native route");
    let accounts = parsed.snapshot_addresses().expect("native accounts").iter().map(|key| json!({"address": key.to_string(), "owner": address(1), "lamports": "1", "executable": false, "dataBase64": ""})).collect::<Vec<_>>();
    json!({"format": "dclutch-general-plan-input-v1", "route": route, "snapshotSlot": "77", "snapshotUnixTimestamp": "1800000000", "recentBlockhash": address(2), "accounts": accounts})
}

#[test]
fn browser_route_acquisition_is_the_production_cli_parsers_account_set_for_every_action() {
    for action in [
        Action::Consider,
        Action::Freeze,
        Action::InitializeSettlement,
        Action::Collect,
        Action::Materialize,
        Action::Distribute,
        Action::Close,
        Action::OpenBatch,
        Action::PlaceOrder,
        Action::CancelOrder,
        Action::CloseBatch,
        Action::SubmitCandidate,
        Action::VerifyCandidateRow,
        Action::ReleaseOrder,
        Action::CloseCandidate,
    ] {
        let text = route(action);
        let native = producer::parse_route_v1(text.as_bytes()).expect("native route");
        let result: Value =
            serde_json::from_str(&general_route_accounts_json_v1(&text).expect("browser route"))
                .expect("result JSON");
        assert_eq!(result["minimumFinalizedSlot"], "77");
        assert_eq!(
            result["addresses"],
            json!(
                native
                    .snapshot_addresses()
                    .expect("native snapshot")
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            )
        );
    }
}

#[test]
fn canonical_observation_preserves_the_production_producers_exact_first_refusal() {
    let input = input();
    let route = producer::parse_route_v1(input["route"].as_str().expect("route").as_bytes())
        .expect("native route");
    let accounts = route
        .snapshot_addresses()
        .expect("accounts")
        .into_iter()
        .map(|key| ObservedAccount {
            observation: Observation {
                slot: 77,
                unix_timestamp: 1_800_000_000,
                finality: Finality::Finalized,
            },
            key,
            owner: Pubkey::new_from_array([1; 32]),
            lamports: 1,
            executable: false,
            data: Vec::new(),
        })
        .collect();
    let native = producer::produce_plan_v5(&route, accounts, Hash::new_from_array([2; 32]))
        .expect_err("empty corpus is not an admitted plan")
        .to_string();
    assert_eq!(plan_general_json_v1(&input.to_string()), Err(native));
}

#[test]
fn transport_refuses_stale_duplicate_substituted_and_noncanonical_observations_before_native_execution()
 {
    let mut value = input();
    value["snapshotSlot"] = json!("76");
    assert_eq!(
        plan_general_json_v1(&value.to_string()),
        Err("General snapshot precedes the route floor".to_owned())
    );
    let mut value = input();
    value["accounts"][1] = value["accounts"][0].clone();
    assert_eq!(
        plan_general_json_v1(&value.to_string()),
        Err("General observation substituted or duplicated an account".to_owned())
    );
    let mut value = input();
    value["accounts"][0]["address"] = json!(address(200));
    assert_eq!(
        plan_general_json_v1(&value.to_string()),
        Err("General observation substituted or duplicated an account".to_owned())
    );
    let mut value = input();
    value["accounts"][0]["lamports"] = json!("01");
    assert_eq!(
        plan_general_json_v1(&value.to_string()),
        Err("account lamports is not canonical u64".to_owned())
    );
    let mut value = input();
    value["accounts"]
        .as_array_mut()
        .expect("account array")
        .pop();
    assert_eq!(
        plan_general_json_v1(&value.to_string()),
        Err("General observation account count differs from the native route".to_owned())
    );
}
