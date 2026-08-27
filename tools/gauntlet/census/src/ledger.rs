//! Folding campaign chain-evidence into the execution ledger.
//!
//! The ledger's whole value is that it records what the CHAIN says ran, not
//! what the harness believes it submitted. Every observation is cross-checked
//! against the finalized transaction's own log messages before it is admitted,
//! and a campaign transaction with no binding is a hard error rather than a
//! silent skip.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::model::{
    Binding, Bindings, Inventory, LEDGER_SCHEMA_V1, Ledger, Observation, Outcome, ProgramMap,
};

pub struct FoldReport {
    pub admitted: usize,
    pub problems: Vec<String>,
}

/// One finalized transaction as the campaign recorded it.
struct CampaignTransaction {
    label: String,
    signature: String,
    slot: u64,
    failed: bool,
    error: Option<String>,
    compute_units: Option<u64>,
    logs: Vec<String>,
}

fn read_transactions(evidence: &Value) -> Result<Vec<CampaignTransaction>, String> {
    let array = evidence
        .get("transactions")
        .and_then(Value::as_array)
        .ok_or("campaign evidence has no `transactions` array")?;
    let mut found = Vec::with_capacity(array.len());
    for entry in array {
        let label = entry
            .get("label")
            .and_then(Value::as_str)
            .ok_or("campaign transaction omitted `label`")?
            .to_string();
        let signature = entry
            .get("signature")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let slot = entry.get("slot").and_then(Value::as_u64).unwrap_or(0);
        let error = entry
            .get("error")
            .filter(|value| !value.is_null())
            .map(ToString::to_string);
        let logs = entry
            .get("logs")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        found.push(CampaignTransaction {
            label,
            signature,
            slot,
            failed: error.is_some(),
            error,
            compute_units: entry.get("compute_units_consumed").and_then(Value::as_u64),
            logs,
        });
    }
    Ok(found)
}

/// Program addresses the chain's own log messages report as invoked.
fn programs_invoked(logs: &[String]) -> Vec<String> {
    let mut found = Vec::new();
    for line in logs {
        let Some(rest) = line.strip_prefix("Program ") else {
            continue;
        };
        let Some((address, tail)) = rest.split_once(' ') else {
            continue;
        };
        if !tail.starts_with("invoke [") {
            continue;
        }
        if !found.iter().any(|held| held == address) {
            found.push(address.to_string());
        }
    }
    found
}

/// The `custom program error: 0xN` the chain reported, if any.
fn reported_custom_code(logs: &[String], error: Option<&str>) -> Option<u64> {
    for line in logs.iter().rev() {
        if let Some(index) = line.find("custom program error: 0x") {
            let hex: String = line[index + "custom program error: 0x".len()..]
                .chars()
                .take_while(char::is_ascii_hexdigit)
                .collect();
            if let Ok(value) = u64::from_str_radix(&hex, 16) {
                return Some(value);
            }
        }
    }
    // Fall back to the structured error, e.g. {"InstructionError":[0,{"Custom":3}]}.
    let error = error?;
    let marker = "\"Custom\":";
    let index = error.find(marker)?;
    let digits: String = error[index + marker.len()..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// `*` matches any run of characters, anywhere in the pattern. Deliberately the
/// only metacharacter: a binding pattern is read by a human deciding whether a
/// campaign step is covered, and a regex would make that harder, not easier.
fn matches_label(pattern: &str, label: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == label;
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut rest = label;
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if index == 0 {
            // The first literal is anchored at the start.
            let Some(tail) = rest.strip_prefix(part) else {
                return false;
            };
            rest = tail;
        } else if index + 1 == parts.len() {
            // The last literal is anchored at the end.
            return rest.len() >= part.len() && rest.ends_with(part);
        } else {
            let Some(position) = rest.find(part) else {
                return false;
            };
            rest = &rest[position + part.len()..];
        }
    }
    true
}

/// Fold a campaign's evidence document into the ledger.
///
/// Returns the problems found. An empty `problems` list is the only outcome
/// that should let a gate pass.
#[allow(clippy::too_many_lines)]
pub fn fold(
    ledger: &mut Ledger,
    inventory: &Inventory,
    bindings: &Bindings,
    programs: &ProgramMap,
    evidence: &Value,
    evidence_path: &str,
    evidence_bytes: &[u8],
) -> Result<FoldReport, String> {
    if ledger.schema.is_empty() {
        ledger.schema = LEDGER_SCHEMA_V1.into();
    } else if ledger.schema != LEDGER_SCHEMA_V1 {
        return Err(format!("unsupported ledger schema: {}", ledger.schema));
    }

    let known_routes: BTreeSet<&str> = inventory
        .programs
        .iter()
        .flat_map(|program| program.routes.iter())
        .map(|route| route.id.as_str())
        .collect();
    let known_refusals: BTreeMap<&str, Option<i64>> = inventory
        .programs
        .iter()
        .flat_map(|program| program.refusals.iter())
        .map(|refusal| (refusal.id.as_str(), refusal.code))
        .collect();

    let mut problems = Vec::new();
    for binding in &bindings.bindings {
        for route in &binding.routes {
            if !known_routes.contains(route.as_str()) {
                problems.push(format!(
                    "binding `{}` names route `{route}`, which is not in the inventory",
                    binding.label
                ));
            }
        }
        // An empty program label is the explicit, honest form for a
        // transaction that drives no protocol route at all: an airdrop, a
        // Loader SetAuthority, an Address Lookup Table extension. It must
        // therefore claim no routes either.
        if binding.program.is_empty() {
            if !binding.routes.is_empty() {
                problems.push(format!(
                    "binding `{}` claims routes but names no program; a route claim the chain \
                     cannot corroborate is not admissible",
                    binding.label
                ));
            }
        } else if !programs.contains_key(&binding.program) {
            problems.push(format!(
                "binding `{}` names program label `{}`, which the program map does not carry",
                binding.label, binding.program
            ));
        }
        if binding.outcome == Outcome::Refused {
            match (&binding.refusal, &binding.unnamed_refusal) {
                (None, None) => problems.push(format!(
                    "binding `{}` expects a refusal but names no census refusal id",
                    binding.label
                )),
                (Some(_), Some(_)) => problems.push(format!(
                    "binding `{}` names both a census refusal and an unnamed one; \
                     a refusal has exactly one account of where it came from",
                    binding.label
                )),
                (Some(refusal), None) if !known_refusals.contains_key(refusal.as_str()) => {
                    problems.push(format!(
                        "binding `{}` names refusal `{refusal}`, which is not in the inventory",
                        binding.label
                    ));
                }
                (Some(_), None) => {}
                (None, Some(unnamed)) if unnamed.reason.trim().is_empty() => {
                    problems.push(format!(
                        "binding `{}` credits its refusal to no census code but does not say \
                         which program raised it; an uncredited refusal with no reason is how \
                         a real refusal launders itself out of the taxonomy",
                        binding.label
                    ));
                }
                (None, Some(_)) => {}
            }
        }
    }

    let evidence_sha256 = hex(&Sha256::digest(evidence_bytes));
    let transactions = read_transactions(evidence)?;
    let mut admitted = 0_usize;
    let mut used: Vec<&Binding> = Vec::new();

    for transaction in &transactions {
        let matching: Vec<&Binding> = bindings
            .bindings
            .iter()
            .filter(|binding| matches_label(&binding.label, &transaction.label))
            .collect();
        if matching.is_empty() {
            problems.push(format!(
                "campaign transaction `{}` has no census binding \
                 (unbound labels are how coverage silently rots)",
                transaction.label
            ));
            continue;
        }
        if matching.len() > 1 {
            problems.push(format!(
                "campaign transaction `{}` matched {} bindings; bindings must be unambiguous",
                transaction.label,
                matching.len()
            ));
            continue;
        }
        let binding = matching[0];
        if !used.iter().any(|held| std::ptr::eq(*held, binding)) {
            used.push(binding);
        }

        let invoked = programs_invoked(&transaction.logs);
        if !binding.program.is_empty() {
            let Some(expected_address) = programs.get(&binding.program) else {
                continue;
            };
            if !invoked.iter().any(|address| address == expected_address) {
                problems.push(format!(
                    "`{}` claims to drive {} ({expected_address}) but the finalized logs show only [{}] \
                     — the chain does not corroborate the route",
                    transaction.label,
                    binding.program,
                    invoked.join(", ")
                ));
                continue;
            }
        }

        let observed_outcome = if transaction.failed {
            Outcome::Refused
        } else {
            Outcome::Executed
        };
        if observed_outcome != binding.outcome {
            problems.push(format!(
                "`{}` was bound as {:?} but the chain reports {observed_outcome:?}",
                transaction.label, binding.outcome
            ));
            continue;
        }

        let mut refusal = None;
        if observed_outcome == Outcome::Refused
            && let Some(unnamed) = binding.unnamed_refusal.as_ref()
        {
            // The code is still checked against the chain; it is simply not
            // credited to any enumerated program's taxonomy.
            let reported = reported_custom_code(&transaction.logs, transaction.error.as_deref());
            if reported != Some(u64::from(unnamed.code)) {
                problems.push(format!(
                    "`{}` expects the uncredited refusal {} ({}) but the chain reported {}",
                    transaction.label,
                    unnamed.code,
                    unnamed.reason,
                    reported.map_or_else(|| "no custom program error".to_owned(), |code| code
                        .to_string())
                ));
                continue;
            }
        } else if observed_outcome == Outcome::Refused {
            let expected = binding.refusal.as_deref().unwrap_or_default();
            let expected_code = known_refusals.get(expected).copied().flatten();
            let reported = reported_custom_code(&transaction.logs, transaction.error.as_deref());
            match (expected_code, reported) {
                (Some(expected_code), Some(reported)) => {
                    if i64::try_from(reported) == Ok(expected_code) {
                        refusal = Some(expected.to_string());
                    } else {
                        problems.push(format!(
                            "`{}` expected {expected} (code {expected_code}) but the chain \
                             reported custom program error {reported}",
                            transaction.label
                        ));
                        continue;
                    }
                }
                (_, None) => {
                    // A refusal raised before the program's own error taxonomy
                    // (a runtime privilege/frame refusal) is recorded honestly
                    // as an unnamed refusal rather than credited to a code.
                    refusal = None;
                }
                (None, Some(reported)) => {
                    problems.push(format!(
                        "`{}` names refusal {expected}, which carries no numeric code, \
                         while the chain reported custom program error {reported}",
                        transaction.label
                    ));
                    continue;
                }
            }
        }

        for route in &binding.routes {
            ledger.observations.push(Observation {
                route: route.clone(),
                campaign: bindings.campaign.clone(),
                label: transaction.label.clone(),
                signature: transaction.signature.clone(),
                slot: transaction.slot,
                outcome: observed_outcome,
                refusal: refusal.clone(),
                compute_units: transaction.compute_units,
                programs_invoked: invoked.clone(),
                evidence_sha256: evidence_sha256.clone(),
                evidence_path: evidence_path.to_string(),
            });
            admitted += 1;
        }
    }

    for binding in &bindings.bindings {
        if !used.iter().any(|held| std::ptr::eq(*held, binding)) {
            problems.push(format!(
                "binding `{}` matched no transaction in this campaign \
                 (a stale binding overstates coverage)",
                binding.label
            ));
        }
    }

    // Keep the ledger canonically ordered and free of exact duplicates.
    ledger.observations.sort_by(|left, right| {
        (&left.route, left.slot, &left.signature).cmp(&(&right.route, right.slot, &right.signature))
    });
    ledger
        .observations
        .dedup_by(|left, right| left.route == right.route && left.signature == right.signature);

    Ok(FoldReport { admitted, problems })
}

pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        Binding, Inventory, ProgramSurface, Refusal, Route, RouteKind, UnnamedRefusal,
    };
    use serde_json::json;

    fn inventory() -> Inventory {
        Inventory {
            schema: crate::model::INVENTORY_SCHEMA_V1.into(),
            source_root: "/tmp".into(),
            source_revision: None,
            programs: vec![ProgramSurface {
                package: "dclutch-core-sbf".into(),
                label: "core".into(),
                crate_root: "programs/dclutch-core-sbf/src/lib.rs".into(),
                entrypoints: Vec::new(),
                routes: vec![Route {
                    id: "core/found::process#Found".into(),
                    kind: RouteKind::Entry,
                    parent: None,
                    handler: "found::process".into(),
                    selectors: Vec::new(),
                    provenance: "programs/dclutch-core-sbf/src/lib.rs:252".into(),
                    cfg: Vec::new(),
                }],
                refusals: vec![Refusal {
                    id: "core/CoreSbfError::RentCredit".into(),
                    enum_name: "CoreSbfError".into(),
                    variant: "RentCredit".into(),
                    code: Some(6),
                    doc: None,
                    provenance: "programs/dclutch-core-sbf/src/lib.rs:99".into(),
                }],
                unclassified: Vec::new(),
            }],
        }
    }

    fn programs() -> ProgramMap {
        let mut map = ProgramMap::new();
        map.insert("core".into(), "CoreProgram1111".into());
        map
    }

    fn bindings(binding: Binding) -> Bindings {
        Bindings {
            campaign: "tier1".into(),
            note: String::new(),
            bindings: vec![binding],
        }
    }

    fn executed_binding() -> Binding {
        Binding {
            label: "create canonical Found31 Market".into(),
            routes: vec!["core/found::process#Found".into()],
            program: "core".into(),
            outcome: Outcome::Executed,
            refusal: None,
            unnamed_refusal: None,
            note: String::new(),
        }
    }

    fn evidence(transactions: &serde_json::Value) -> Value {
        json!({ "transactions": transactions })
    }

    fn success(label: &str, program: &str) -> Value {
        json!({
            "label": label,
            "signature": "sig1",
            "slot": 7,
            "error": null,
            "compute_units_consumed": 234_043,
            "logs": [format!("Program {program} invoke [1]"), format!("Program {program} success")]
        })
    }

    fn run(bindings: &Bindings, evidence: &Value) -> FoldReport {
        let mut ledger = Ledger::default();
        fold(
            &mut ledger,
            &inventory(),
            bindings,
            &programs(),
            evidence,
            "evidence.json",
            b"{}",
        )
        .expect("fold")
    }

    #[test]
    fn label_globs_match_only_what_they_name() {
        assert!(matches_label(
            "publish record: Begin",
            "publish record: Begin"
        ));
        assert!(!matches_label(
            "publish record: Begin",
            "publish record: Append"
        ));
        assert!(matches_label(
            "publish Product graph: *Begin",
            "publish Product graph: ResultDomain Begin"
        ));
        assert!(!matches_label(
            "publish Product graph: *Begin",
            "publish Product graph: ResultDomain Append"
        ));
        assert!(matches_label(
            "extend * routing table page *",
            "extend product/sol-usd-range-protection routing table page 2"
        ));
        assert!(!matches_label(
            "extend * routing table page *",
            "create product/sol-usd-range-protection routing address lookup table"
        ));
        assert!(matches_label(
            "activate immutable release-set role: *",
            "activate immutable release-set role: Core"
        ));
    }

    #[test]
    fn a_corroborated_execution_is_admitted() {
        let report = run(
            &bindings(executed_binding()),
            &evidence(&json!([success(
                "create canonical Found31 Market",
                "CoreProgram1111"
            )])),
        );
        assert_eq!(report.admitted, 1);
        assert!(report.problems.is_empty(), "{:?}", report.problems);
    }

    #[test]
    fn an_unbound_campaign_transaction_is_a_problem() {
        // Silence is the failure mode this whole tool exists to remove: a
        // transaction nobody bound must never pass quietly.
        let report = run(
            &bindings(executed_binding()),
            &evidence(&json!([
                success("create canonical Found31 Market", "CoreProgram1111"),
                success("some new step nobody bound", "CoreProgram1111")
            ])),
        );
        assert_eq!(report.admitted, 1);
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.contains("some new step nobody bound")),
            "{:?}",
            report.problems
        );
    }

    #[test]
    fn a_binding_that_matched_nothing_is_a_problem() {
        let report = run(&bindings(executed_binding()), &evidence(&json!([])));
        assert_eq!(report.admitted, 0);
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.contains("matched no transaction")),
            "{:?}",
            report.problems
        );
    }

    #[test]
    fn a_route_the_chain_does_not_corroborate_is_refused() {
        // The harness says Core ran. The finalized logs say only the System
        // Program ran. The chain wins, and no observation is recorded.
        let report = run(
            &bindings(executed_binding()),
            &evidence(&json!([success(
                "create canonical Found31 Market",
                "11111111111111111111111111111111"
            )])),
        );
        assert_eq!(report.admitted, 0);
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.contains("does not corroborate")),
            "{:?}",
            report.problems
        );
    }

    #[test]
    fn an_unknown_route_id_is_a_problem() {
        let mut binding = executed_binding();
        binding.routes = vec!["core/found::process#Renamed".into()];
        let report = run(
            &bindings(binding),
            &evidence(&json!([success(
                "create canonical Found31 Market",
                "CoreProgram1111"
            )])),
        );
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.contains("not in the inventory")),
            "{:?}",
            report.problems
        );
    }

    #[test]
    fn an_outcome_the_chain_contradicts_is_refused() {
        let refusal = json!([{
            "label": "create canonical Found31 Market",
            "signature": "sig1",
            "slot": 7,
            "error": {"InstructionError": [0, {"Custom": 6}]},
            "compute_units_consumed": 6_958,
            "logs": ["Program CoreProgram1111 invoke [1]",
                     "Program CoreProgram1111 failed: custom program error: 0x6"]
        }]);
        let report = run(&bindings(executed_binding()), &evidence(&refusal));
        assert_eq!(report.admitted, 0);
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.contains("chain reports")),
            "{:?}",
            report.problems
        );
    }

    #[test]
    fn a_refusal_from_outside_the_census_is_checked_but_never_credited() {
        // A test-only caller that refuses AFTER the child committed reports its
        // own code. Here that code is 6, which collides exactly with
        // `core/CoreSbfError::RentCredit`. Crediting the collision would make
        // the census claim Core raised a refusal it never raised.
        let refused = json!([{
            "label": "caller refuses after Found31 committed",
            "signature": "sig9",
            "slot": 11,
            "error": {"InstructionError": [0, {"Custom": 6}]},
            "compute_units_consumed": 12_345,
            "logs": ["Program CoreProgram1111 invoke [1]",
                     "Program CoreProgram1111 success",
                     "Program TestCaller11111 failed: custom program error: 0x6"]
        }]);

        let mut lying = executed_binding();
        lying.label = "caller refuses after Found31 committed".into();
        lying.outcome = Outcome::Refused;
        lying.refusal = Some("core/CoreSbfError::RentCredit".into());
        let report = run(&bindings(lying), &evidence(&refused));
        // The census cannot tell this apart from a real Core refusal, which is
        // exactly why the honest form has to be available and used.
        assert_eq!(report.admitted, 1);

        let mut both = executed_binding();
        both.label = "caller refuses after Found31 committed".into();
        both.outcome = Outcome::Refused;
        both.refusal = Some("core/CoreSbfError::RentCredit".into());
        both.unnamed_refusal = Some(UnnamedRefusal {
            code: 6,
            reason: "the test caller".into(),
        });
        let report = run(&bindings(both), &evidence(&refused));
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.contains("exactly one account")),
            "{:?}",
            report.problems
        );

        let mut silent = executed_binding();
        silent.label = "caller refuses after Found31 committed".into();
        silent.outcome = Outcome::Refused;
        silent.refusal = None;
        silent.unnamed_refusal = Some(UnnamedRefusal {
            code: 6,
            reason: "   ".into(),
        });
        let report = run(&bindings(silent), &evidence(&refused));
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.contains("launders itself")),
            "{:?}",
            report.problems
        );

        let mut wrong_code = executed_binding();
        wrong_code.label = "caller refuses after Found31 committed".into();
        wrong_code.outcome = Outcome::Refused;
        wrong_code.refusal = None;
        wrong_code.unnamed_refusal = Some(UnnamedRefusal {
            code: 3,
            reason: "the test-only caller's DeliberateLateFailure".into(),
        });
        let report = run(&bindings(wrong_code), &evidence(&refused));
        assert_eq!(report.admitted, 0);
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.contains("uncredited refusal 3")),
            "{:?}",
            report.problems
        );

        let mut honest = executed_binding();
        honest.label = "caller refuses after Found31 committed".into();
        honest.outcome = Outcome::Refused;
        honest.refusal = None;
        honest.unnamed_refusal = Some(UnnamedRefusal {
            code: 6,
            reason: "the test-only caller's DeliberateLateFailure, which is not \
                     a Core refusal despite sharing its number"
                .into(),
        });
        let mut ledger = Ledger::default();
        let report = fold(
            &mut ledger,
            &inventory(),
            &bindings(honest),
            &programs(),
            &evidence(&refused),
            "evidence.json",
            b"{}",
        )
        .expect("fold");
        assert!(report.problems.is_empty(), "{:?}", report.problems);
        assert_eq!(report.admitted, 1);
        // Recorded as a refusal, credited to no first-party code.
        assert_eq!(ledger.observations.len(), 1);
        assert_eq!(ledger.observations[0].outcome, Outcome::Refused);
        assert_eq!(ledger.observations[0].refusal, None);
    }

    #[test]
    fn the_named_refusal_must_be_the_refusal_the_chain_raised() {
        let mut binding = executed_binding();
        binding.label = "Found31 refuses substituted lifecycle credit".into();
        binding.outcome = Outcome::Refused;
        binding.refusal = Some("core/CoreSbfError::RentCredit".into());
        // The chain raises 0x7 (Creation), not the 0x6 (RentCredit) the
        // binding names. "It refused" is not the same claim as "it refused
        // for this reason".
        let wrong = json!([{
            "label": "Found31 refuses substituted lifecycle credit",
            "signature": "sig2",
            "slot": 8,
            "error": {"InstructionError": [0, {"Custom": 7}]},
            "compute_units_consumed": 6_958,
            "logs": ["Program CoreProgram1111 invoke [1]",
                     "Program CoreProgram1111 failed: custom program error: 0x7"]
        }]);
        let report = run(&bindings(binding.clone()), &evidence(&wrong));
        assert_eq!(report.admitted, 0);
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.contains("custom program error 7")),
            "{:?}",
            report.problems
        );

        let right = json!([{
            "label": "Found31 refuses substituted lifecycle credit",
            "signature": "sig2",
            "slot": 8,
            "error": {"InstructionError": [0, {"Custom": 6}]},
            "compute_units_consumed": 6_958,
            "logs": ["Program CoreProgram1111 invoke [1]",
                     "Program CoreProgram1111 failed: custom program error: 0x6"]
        }]);
        let report = run(&bindings(binding), &evidence(&right));
        assert_eq!(report.admitted, 1);
        assert!(report.problems.is_empty(), "{:?}", report.problems);
    }

    #[test]
    fn a_refusal_binding_naming_no_refusal_is_a_problem() {
        let mut binding = executed_binding();
        binding.outcome = Outcome::Refused;
        binding.refusal = None;
        let report = run(
            &bindings(binding),
            &evidence(&json!([success(
                "create canonical Found31 Market",
                "CoreProgram1111"
            )])),
        );
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.contains("names no census refusal id")),
            "{:?}",
            report.problems
        );
    }

    #[test]
    fn program_invocation_is_read_from_the_logs_not_the_binding() {
        let logs = vec![
            "Program CoreProgram1111 invoke [1]".to_string(),
            "Program RegistryProgram1 invoke [2]".to_string(),
            "Program RegistryProgram1 consumed 531543 of 537635 compute units".to_string(),
            "Program CoreProgram1111 success".to_string(),
        ];
        assert_eq!(
            programs_invoked(&logs),
            vec![
                "CoreProgram1111".to_string(),
                "RegistryProgram1".to_string()
            ]
        );
        // A `consumed` line is not an invocation.
        assert!(!programs_invoked(&logs).contains(&"consumed".to_string()));
    }

    #[test]
    fn a_runtime_refusal_before_the_program_taxonomy_is_not_credited_to_a_code() {
        let mut binding = executed_binding();
        binding.outcome = Outcome::Refused;
        binding.refusal = Some("core/CoreSbfError::RentCredit".into());
        let runtime_refusal = evidence(&json!([{
            "label": "create canonical Found31 Market",
            "signature": "sig3",
            "slot": 9,
            "error": {"InstructionError": [0, "PrivilegeEscalation"]},
            "compute_units_consumed": 0,
            "logs": ["Program CoreProgram1111 invoke [1]",
                     "Program CoreProgram1111 failed: Cross-program invocation with unauthorized signer"]
        }]));
        let mut ledger = Ledger::default();
        let report = fold(
            &mut ledger,
            &inventory(),
            &bindings(binding),
            &programs(),
            &runtime_refusal,
            "evidence.json",
            b"{}",
        )
        .expect("fold");
        assert!(report.problems.is_empty(), "{:?}", report.problems);
        assert_eq!(ledger.observations.len(), 1);
        assert_eq!(ledger.observations[0].outcome, Outcome::Refused);
        assert_eq!(
            ledger.observations[0].refusal, None,
            "crediting a runtime refusal to a program's error code would overstate what the program proved"
        );
    }
}
