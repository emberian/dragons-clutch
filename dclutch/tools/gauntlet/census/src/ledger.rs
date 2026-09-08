//! Folding campaign chain-evidence into the execution ledger.
//!
//! The ledger's whole value is that it records what the CHAIN says ran, not
//! what the harness believes it submitted. Every observation is cross-checked
//! against the finalized transaction's own evidence before it is admitted.
//! Logs authenticate the program frame, while authenticated instruction bytes
//! are matched against the inventory's native selectors to authenticate the
//! route within that program. A campaign transaction with no binding is a hard
//! error rather than a silent skip.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::model::{
    Binding, Bindings, EvidenceLevel, Inventory, LEDGER_SCHEMA_V1, Ledger, NativeVariantSelector,
    Observation, Outcome, ProgramMap, Route, Selector,
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
    /// Instructions recovered from the finalized transaction packet and its
    /// finalized CPI metadata. Generic invoke/success logs cannot distinguish
    /// two routes in one program, so this is required before a route claim is
    /// admitted.
    instructions: Option<Vec<CampaignInstruction>>,
}

struct CampaignInstruction {
    program_id: String,
    data_hex: String,
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
        let instructions = match entry.get("instructions") {
            None | Some(Value::Null) => None,
            Some(value) => {
                let values = value
                    .as_array()
                    .ok_or("campaign transaction `instructions` is not an array")?;
                let mut found = Vec::with_capacity(values.len());
                for value in values {
                    let program_id = value
                        .get("program_id")
                        .and_then(Value::as_str)
                        .ok_or("campaign instruction omitted `program_id`")?
                        .to_owned();
                    let data_hex = value
                        .get("data_hex")
                        .and_then(Value::as_str)
                        .ok_or("campaign instruction omitted `data_hex`")?
                        .to_owned();
                    found.push(CampaignInstruction {
                        program_id,
                        data_hex,
                    });
                }
                Some(found)
            }
        };
        found.push(CampaignTransaction {
            label,
            signature,
            slot,
            failed: error.is_some(),
            error,
            compute_units: entry.get("compute_units_consumed").and_then(Value::as_u64),
            logs,
            instructions,
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

/// The custom program error the chain reported, and the program that raised it.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ReportedRefusal {
    code: u64,
    /// Address of the program whose own `failed:` line originated the code, if
    /// the logs name one.
    program: Option<String>,
}

/// Parse one `Program <id> failed: custom program error: 0xN` line.
fn failed_line(line: &str) -> Option<(String, u64)> {
    let rest = line.strip_prefix("Program ")?;
    let (address, tail) = rest.split_once(" failed: ")?;
    let hex = tail.strip_prefix("custom program error: 0x")?;
    let digits: String = hex.chars().take_while(char::is_ascii_hexdigit).collect();
    let code = u64::from_str_radix(&digits, 16).ok()?;
    Some((address.to_string(), code))
}

/// The `custom program error: 0xN` the chain reported, with its program.
///
/// The code is the LAST one in the log: that is what the transaction error
/// carries, and a frame that catches a child's refusal and raises its own has
/// the last word. The program is the FIRST frame to report that code, because
/// a propagated refusal is re-reported by every frame it unwinds through and
/// only the innermost one originated it.
///
/// Attribution is the whole point. Before it, this function returned a bare
/// number and the fold credited it to whichever first-party refusal shared it
/// -- so a test caller's deliberate late failure could be recorded as coverage
/// of a Claims refusal it had nothing to do with. Namespacing the codes
/// (decision 0007) makes that collision impossible by construction; parsing
/// the program off the line makes the census stop depending on that.
fn reported_custom_code(logs: &[String], error: Option<&str>) -> Option<ReportedRefusal> {
    let reported: Vec<(String, u64)> = logs.iter().filter_map(|line| failed_line(line)).collect();
    if let Some(code) = reported.last().map(|(_, code)| *code) {
        let program = reported
            .iter()
            .find(|(_, held)| *held == code)
            .map(|(address, _)| address.clone());
        return Some(ReportedRefusal { code, program });
    }

    // A log line without the `Program <id> failed:` prefix still carries the
    // code; take it, and say honestly that no program was named.
    for line in logs.iter().rev() {
        if let Some(index) = line.find("custom program error: 0x") {
            let hex: String = line[index + "custom program error: 0x".len()..]
                .chars()
                .take_while(char::is_ascii_hexdigit)
                .collect();
            if let Ok(code) = u64::from_str_radix(&hex, 16) {
                return Some(ReportedRefusal {
                    code,
                    program: None,
                });
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
    digits.parse().ok().map(|code| ReportedRefusal {
        code,
        program: None,
    })
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

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if value.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    let mut chars = value.chars();
    while let (Some(high), Some(low)) = (chars.next(), chars.next()) {
        let high = high.to_digit(16)?;
        let low = low.to_digit(16)?;
        bytes.push(((high << 4) | low) as u8);
    }
    Some(bytes)
}

/// Match the two Relay V1 variants whose finalized Ensemble campaign is being
/// folded into the native ledger.
///
/// The inventory names these inner dispatch arms by enum variant rather than
/// repeating the outer `DCLTRIX1` magic.  Their native discriminant is still
/// present in the signed instruction at the Relay V1 action offset, so this is
/// an exact byte check rather than an inference from a same-program log.  Keep
/// this table deliberately closed: another variant needs its own evidence and
/// test before the census can credit it.
fn relay_v1_variant_selected(path: &str, data: &[u8]) -> bool {
    const MAGIC: &[u8; 8] = b"DCLTRIX1";
    const SCHEMA_VERSION: &[u8; 2] = &[1, 0];
    const ACTION_OFFSET: usize = 10;
    const HEADER_RESERVED: core::ops::Range<usize> = 11..16;
    const TERMINAL_SEQUENCE: core::ops::Range<usize> = 24..32;
    const ENSEMBLE_FOLD_BYTES: usize = 32;
    const RECLAIM_MEMBER_SEAT_BYTES: usize = 40;
    const RECLAIM_RESERVED: core::ops::Range<usize> = 33..40;

    let (action, width) = match path {
        "RelayInstructionV1::EnsembleFold" => (8, ENSEMBLE_FOLD_BYTES),
        "RelayInstructionV1::ReclaimMemberSeat" => (9, RECLAIM_MEMBER_SEAT_BYTES),
        _ => return false,
    };
    data.len() == width
        && data.get(..MAGIC.len()) == Some(MAGIC)
        && data.get(8..10) == Some(SCHEMA_VERSION)
        && data.get(ACTION_OFFSET) == Some(&action)
        && data
            .get(HEADER_RESERVED)
            .is_some_and(|bytes| bytes.iter().all(|byte| *byte == 0))
        && data
            .get(TERMINAL_SEQUENCE)
            .is_some_and(|bytes| bytes.iter().any(|byte| *byte != 0))
        && (path != "RelayInstructionV1::ReclaimMemberSeat"
            || data
                .get(RECLAIM_RESERVED)
                .is_some_and(|bytes| bytes.iter().all(|byte| *byte == 0)))
}

/// Match the action-specific half of the shared native claim-check redemption
/// magic. The source decoder requires this exact width/header/body before it
/// enters `process_escrow_close`; a matching magic by itself would also admit
/// the holder-signed redemption route.
fn claim_check_v1_variant_selected(path: &str, data: &[u8]) -> bool {
    const MAGIC: &[u8; 8] = b"DCLTCCR1";
    const SCHEMA_VERSION: &[u8; 2] = &[1, 0];
    const ACTION_OFFSET: usize = 10;
    const CLOSE_ESCROW_ACTION: u8 = 4;
    const HEADER_RESERVED: core::ops::Range<usize> = 11..16;
    const AGGREGATE: core::ops::Range<usize> = 16..48;
    const BODY_RESERVED: core::ops::Range<usize> = 48..64;
    const CLOSE_ESCROW_BYTES: usize = 64;

    path == "dclutch_claims::claim_check_request_v1::ClaimCheckActionV1::CloseEscrow"
        && data.len() == CLOSE_ESCROW_BYTES
        && data.get(..MAGIC.len()) == Some(MAGIC)
        && data.get(8..10) == Some(SCHEMA_VERSION)
        && data.get(ACTION_OFFSET) == Some(&CLOSE_ESCROW_ACTION)
        && data
            .get(HEADER_RESERVED)
            .is_some_and(|bytes| bytes.iter().all(|byte| *byte == 0))
        && data
            .get(AGGREGATE)
            .is_some_and(|bytes| bytes.iter().any(|byte| *byte != 0))
        && data
            .get(BODY_RESERVED)
            .is_some_and(|bytes| bytes.iter().all(|byte| *byte == 0))
}

/// Match a source-derived Fractional Exposure V2 action selector against the
/// exact canonical request header that reaches Claims' inner action dispatch.
///
/// The inventory obtains `offset` from the codec's public offset constant and
/// `value` from the enum variant's explicit discriminant. This function adds
/// the surrounding fixed-width/header guards the decoder enforces before the
/// match. It deliberately recognises no other enum family.
pub(crate) const CLAIMS_FRACTIONAL_V2_MAGIC: &[u8; 8] = b"DCFREQ02";
pub(crate) const CLAIMS_FRACTIONAL_V2_SCHEMA_VERSION: u16 = 2;
pub(crate) const CLAIMS_FRACTIONAL_V2_ACTION_OFFSET: usize = 10;
pub(crate) const CLAIMS_FRACTIONAL_V2_HEADER_RESERVED: core::ops::Range<usize> = 11..16;
pub(crate) const CLAIMS_FRACTIONAL_V2_REQUEST_BYTES: usize = 416;
pub(crate) const CLAIMS_FRACTIONAL_V2_TAIL_RESERVED: core::ops::Range<usize> = 388..416;

fn claims_fractional_v2_variant_selected(
    path: &str,
    native: &NativeVariantSelector,
    data: &[u8],
) -> bool {
    path.starts_with("FractionalExposureActionV2::")
        && native.offset == CLAIMS_FRACTIONAL_V2_ACTION_OFFSET
        && data.len() == CLAIMS_FRACTIONAL_V2_REQUEST_BYTES
        && data.get(..CLAIMS_FRACTIONAL_V2_MAGIC.len()) == Some(CLAIMS_FRACTIONAL_V2_MAGIC)
        && data.get(8..10) == Some(&CLAIMS_FRACTIONAL_V2_SCHEMA_VERSION.to_le_bytes())
        && data.get(native.offset) == Some(&native.value)
        && data
            .get(CLAIMS_FRACTIONAL_V2_HEADER_RESERVED)
            .is_some_and(|bytes| bytes.iter().all(|byte| *byte == 0))
        && data
            .get(CLAIMS_FRACTIONAL_V2_TAIL_RESERVED)
            .is_some_and(|bytes| bytes.iter().all(|byte| *byte == 0))
}

/// Whether one finalized instruction's native bytes select this route.
///
/// A route with a selector the census cannot evaluate is deliberately not a
/// match. The caller reports insufficient evidence instead of turning a
/// generic same-program invocation into route coverage.
fn route_selected(route: &Route, program_address: &str, instruction: &CampaignInstruction) -> bool {
    // Enumeration folds every selector seen while reaching one handler into
    // this vector. That vector can contain conjunctive guards, or alternatives
    // from two dispatch arms merged under one route id; the persisted model
    // does not preserve which shape it was. A path-only selector vector is
    // therefore ambiguous. The one exception below is a vector whose every
    // alternative has a source-derived native byte selector in the closed
    // Claims family.
    if instruction.program_id != program_address || route.selectors.is_empty() {
        return false;
    }
    let Some(data) = decode_hex(&instruction.data_hex) else {
        return false;
    };

    // Several source variants may dispatch to one handler. When every one has
    // a source-derived byte selector in the same closed native family, they
    // are alternatives: one exact action byte selects the handler. Any mixed
    // or path-only vector still fails closed below.
    if route.selectors.len() > 1
        && route.selectors.iter().all(|selector| {
            matches!(
                selector,
                Selector::Variant {
                    native: Some(_),
                    ..
                }
            )
        })
    {
        return route.selectors.iter().any(|selector| match selector {
            Selector::Variant {
                path,
                native: Some(native),
            } => claims_fractional_v2_variant_selected(path, native, &data),
            _ => false,
        });
    }
    route.selectors.iter().all(|selector| match selector {
        Selector::Magic { bytes, ascii, .. } => {
            let expected = if let Some(ascii) = ascii {
                ascii.as_bytes().to_vec()
            } else if let Some(bytes) = bytes {
                let Some(decoded) = decode_hex(bytes.trim_start_matches("0x")) else {
                    return false;
                };
                decoded
            } else {
                return false;
            };
            !expected.is_empty() && data.starts_with(&expected)
        }
        Selector::Length { value, .. } => value
            .and_then(|value| usize::try_from(value).ok())
            .is_some_and(|value| data.len() == value),
        Selector::Variant { path, native } => native.as_ref().map_or_else(
            || {
                relay_v1_variant_selected(path, &data)
                    || claim_check_v1_variant_selected(path, &data)
            },
            |native| claims_fractional_v2_variant_selected(path, native, &data),
        ),
        // These selectors depend on deserializing the instruction payload or
        // on a predicate body. The census has no native decoder for them, so
        // finalized bytes alone are insufficient to credit this route.
        Selector::Predicate { .. }
        | Selector::Tag { .. }
        | Selector::Literal { .. }
        | Selector::Fallthrough => false,
    })
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

        // A Program <id> invoke/success pair proves only that the program ran.
        // It does not prove which instruction branch ran when two routes share
        // that program. The producer must carry the finalized packet's native
        // instruction bytes (and finalized CPI bytes when available); this
        // census matches those bytes against the inventory selectors below.
        if !binding.routes.is_empty() {
            let Some(instructions) = transaction.instructions.as_ref() else {
                problems.push(format!(
                    "`{}` claims route(s) [{}], but finalized evidence has no native \
                     instruction bytes; route admission has insufficient instruction evidence",
                    transaction.label,
                    binding.routes.join(", ")
                ));
                continue;
            };
            let matched: Vec<&str> = binding
                .routes
                .iter()
                .filter_map(|route_id| {
                    let route = inventory
                        .programs
                        .iter()
                        .flat_map(|program| program.routes.iter())
                        .find(|route| route.id == *route_id)?;
                    let owner = route.id.split('/').next()?;
                    let address = programs.get(owner)?;
                    instructions
                        .iter()
                        .any(|instruction| route_selected(route, address, instruction))
                        .then_some(route_id.as_str())
                })
                .collect();
            let missing: Vec<&str> = binding
                .routes
                .iter()
                .filter(|route| !matched.contains(&route.as_str()))
                .map(String::as_str)
                .collect();
            if !missing.is_empty() {
                problems.push(format!(
                    "`{}` claims route(s) [{}], but finalized native instruction evidence \
                     selected no matching route; the route binding is not corroborated",
                    transaction.label,
                    missing.join(", ")
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
        let mut refusal_program = None;
        if observed_outcome == Outcome::Refused
            && let Some(unnamed) = binding.unnamed_refusal.as_ref()
        {
            // The code is still checked against the chain; it is simply not
            // credited to any enumerated program's taxonomy.
            let reported = reported_custom_code(&transaction.logs, transaction.error.as_deref());
            if reported.as_ref().map(|held| held.code) != Some(u64::from(unnamed.code)) {
                problems.push(format!(
                    "`{}` expects the uncredited refusal {} ({}) but the chain reported {}",
                    transaction.label,
                    unnamed.code,
                    unnamed.reason,
                    reported.as_ref().map_or_else(
                        || "no custom program error".to_owned(),
                        |held| held.code.to_string()
                    )
                ));
                continue;
            }
            refusal_program = reported.and_then(|held| held.program);
        } else if observed_outcome == Outcome::Refused {
            let expected = binding.refusal.as_deref().unwrap_or_default();
            let expected_code = known_refusals.get(expected).copied().flatten();
            let reported = reported_custom_code(&transaction.logs, transaction.error.as_deref());
            match (expected_code, reported) {
                (Some(expected_code), Some(reported)) => {
                    if i64::try_from(reported.code) != Ok(expected_code) {
                        problems.push(format!(
                            "`{}` expected {expected} (code {expected_code}) but the chain \
                             reported custom program error {}",
                            transaction.label, reported.code
                        ));
                        continue;
                    }
                    // The code matching is not enough on its own. A refusal id
                    // is `<program label>/<Enum>::<Variant>`, so the chain has
                    // to agree that THAT program raised it -- otherwise a
                    // number shared with a program the binding never named
                    // gets credited as coverage of a route it never touched.
                    // Namespaced codes make the coincidence impossible; this
                    // check makes the census stop relying on that.
                    let owner = expected.split('/').next().unwrap_or_default();
                    if let (Some(raiser), Some(owner_address)) =
                        (reported.program.as_deref(), programs.get(owner))
                        && raiser != owner_address
                    {
                        problems.push(format!(
                            "`{}` credits {expected} (code {expected_code}), but the chain says \
                             {raiser} raised it, not {owner} ({owner_address}). A refusal is \
                             coverage of the program that raised it or of nothing.",
                            transaction.label
                        ));
                        continue;
                    }
                    refusal = Some(expected.to_string());
                    refusal_program = reported.program;
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
                         while the chain reported custom program error {}",
                        transaction.label, reported.code
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
                refusal_program: refusal_program.clone(),
                compute_units: transaction.compute_units,
                programs_invoked: invoked.clone(),
                evidence_level: EvidenceLevel::FinalizedInstruction,
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
        let level = |observation: &Observation| match observation.evidence_level {
            EvidenceLevel::FinalizedInstruction => 0_u8,
            EvidenceLevel::LegacyProgramOnly => 1_u8,
        };
        (&left.route, left.slot, &left.signature, level(left)).cmp(&(
            &right.route,
            right.slot,
            &right.signature,
            level(right),
        ))
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
        Binding, Inventory, ProgramSurface, Refusal, Route, RouteKind, Selector, UnnamedRefusal,
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
                routes: vec![
                    Route {
                        id: "core/found::process#Found".into(),
                        kind: RouteKind::Entry,
                        parent: None,
                        handler: "found::process".into(),
                        provenance: "programs/dclutch-core-sbf/src/lib.rs:252".into(),
                        cfg: Vec::new(),
                        selectors: vec![Selector::Magic {
                            constant: "FOUND".into(),
                            bytes: Some("01".into()),
                            ascii: None,
                            provenance: None,
                        }],
                        admissible_prestates: Vec::new(),
                        selected_prestates: Vec::new(),
                    },
                    Route {
                        id: "core/found::process#Other".into(),
                        kind: RouteKind::Entry,
                        parent: None,
                        handler: "found::process".into(),
                        provenance: "programs/dclutch-core-sbf/src/lib.rs:253".into(),
                        cfg: Vec::new(),
                        selectors: vec![Selector::Magic {
                            constant: "OTHER".into(),
                            bytes: Some("02".into()),
                            ascii: None,
                            provenance: None,
                        }],
                        admissible_prestates: Vec::new(),
                        selected_prestates: Vec::new(),
                    },
                ],
                refusals: vec![Refusal {
                    id: "core/CoreSbfError::RentCredit".into(),
                    enum_name: "CoreSbfError".into(),
                    variant: "RentCredit".into(),
                    code: Some(6),
                    summary: None,
                    detail: None,
                    provenance: "programs/dclutch-core-sbf/src/lib.rs:99".into(),
                }],
                unclassified: Vec::new(),
                no_persisted_discriminant: None,
            }],
            bands: Vec::new(),
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
            "instructions": [{"program_id": program, "data_hex": "01"}],
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
    fn generic_same_program_logs_cannot_credit_the_wrong_route() {
        // Both sibling routes run in Core and therefore emit the same generic
        // invoke/success transcript. The finalized instruction selects Other,
        // so a Found binding must remain unadmitted.
        let mut wrong = success("create canonical Found31 Market", "CoreProgram1111");
        wrong["instructions"] = json!([{
            "program_id": "CoreProgram1111",
            "data_hex": "02"
        }]);
        let report = run(&bindings(executed_binding()), &evidence(&json!([wrong])));
        assert_eq!(report.admitted, 0);
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.contains("selected no matching route")),
            "{:?}",
            report.problems
        );
    }

    #[test]
    fn missing_finalized_instruction_bytes_are_insufficient_evidence() {
        let mut generic = success("create canonical Found31 Market", "CoreProgram1111");
        generic
            .as_object_mut()
            .expect("transaction object")
            .remove("instructions");
        let report = run(&bindings(executed_binding()), &evidence(&json!([generic])));
        assert_eq!(report.admitted, 0);
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.contains("insufficient instruction evidence")),
            "{:?}",
            report.problems
        );
    }

    #[test]
    fn merged_selector_vectors_are_insufficient_evidence() {
        let mut inventory = inventory();
        inventory.programs[0].routes[0]
            .selectors
            .push(Selector::Magic {
                constant: "ALTERNATIVE".into(),
                bytes: Some("02".into()),
                ascii: None,
                provenance: None,
            });
        let mut ledger = Ledger::default();
        let report = fold(
            &mut ledger,
            &inventory,
            &bindings(executed_binding()),
            &programs(),
            &evidence(&json!([success(
                "create canonical Found31 Market",
                "CoreProgram1111"
            )])),
            "fixture.json",
            b"{}",
        )
        .expect("fold");
        assert_eq!(report.admitted, 0);
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.contains("selected no matching route")),
            "{:?}",
            report.problems
        );
    }

    #[test]
    fn relay_ensemble_variants_require_exact_native_action_and_width() {
        let route = |path: &str| Route {
            id: format!("resolution/process#{path}"),
            kind: RouteKind::Action,
            parent: Some("resolution/relay_transport_v1::process_relay_transport_v1".into()),
            handler: "process".into(),
            provenance: "programs/dclutch-resolution-proof-sbf/src/relay_transport_v1.rs:1".into(),
            cfg: Vec::new(),
            selectors: vec![Selector::Variant {
                path: path.into(),
                native: None,
            }],
            admissible_prestates: Vec::new(),
            selected_prestates: Vec::new(),
        };
        let instruction = |data_hex: &str| CampaignInstruction {
            program_id: "ResolutionProgram1111".into(),
            data_hex: data_hex.into(),
        };

        let fold = route("RelayInstructionV1::EnsembleFold");
        let reclaim = route("RelayInstructionV1::ReclaimMemberSeat");
        // Exact instructions recovered from the finalized accepted packets at
        // retained-ledger slots 20,927 and 21,057.
        let fold_bytes = "44434c5452495831010008000000000002000000000000000100000000000000";
        let reclaim_bytes =
            "44434c54524958310100090000000000020000000000000001000000000000000300000000000000";
        assert!(route_selected(
            &fold,
            "ResolutionProgram1111",
            &instruction(fold_bytes)
        ));
        assert!(route_selected(
            &reclaim,
            "ResolutionProgram1111",
            &instruction(reclaim_bytes)
        ));

        let mut wrong_action = decode_hex(fold_bytes).expect("fold bytes");
        wrong_action[10] = 9;
        assert!(!route_selected(
            &fold,
            "ResolutionProgram1111",
            &instruction(&hex(&wrong_action))
        ));
        let short_reclaim = &reclaim_bytes[..reclaim_bytes.len() - 2];
        assert!(!route_selected(
            &reclaim,
            "ResolutionProgram1111",
            &instruction(short_reclaim)
        ));

        let mut noncanonical = decode_hex(reclaim_bytes).expect("reclaim bytes");
        noncanonical[8] = 2;
        assert!(!route_selected(
            &reclaim,
            "ResolutionProgram1111",
            &instruction(&hex(&noncanonical))
        ));
        noncanonical = decode_hex(reclaim_bytes).expect("reclaim bytes");
        noncanonical[11] = 1;
        assert!(!route_selected(
            &reclaim,
            "ResolutionProgram1111",
            &instruction(&hex(&noncanonical))
        ));
        noncanonical = decode_hex(reclaim_bytes).expect("reclaim bytes");
        noncanonical[33] = 1;
        assert!(!route_selected(
            &reclaim,
            "ResolutionProgram1111",
            &instruction(&hex(&noncanonical))
        ));
    }

    #[test]
    fn claim_check_close_requires_shared_magic_action_and_canonical_packet() {
        let route = Route {
            id: "claims/claim_check_redemption_v1::process_escrow_close#CloseEscrow".into(),
            kind: RouteKind::Action,
            parent: Some("claims/process_non_fractional_instruction".into()),
            handler: "claim_check_redemption_v1::process_escrow_close".into(),
            provenance: "programs/dclutch-claims-sbf/src/lib.rs:1".into(),
            cfg: Vec::new(),
            selectors: vec![
                Selector::Magic {
                    constant: "dclutch_claims::claim_check_v1::CLAIM_CHECK_REDEEM_MAGIC_V1".into(),
                    bytes: Some("44434c5443435231".into()),
                    ascii: Some("DCLTCCR1".into()),
                    provenance: None,
                },
                Selector::Variant {
                    path: "dclutch_claims::claim_check_request_v1::ClaimCheckActionV1::CloseEscrow"
                        .into(),
                    native: None,
                },
            ],
            admissible_prestates: Vec::new(),
            selected_prestates: Vec::new(),
        };
        let instruction = |data: &[u8]| CampaignInstruction {
            program_id: "ClaimsProgram1111".into(),
            data_hex: hex(data),
        };
        let packet = || {
            let mut data = vec![0_u8; 64];
            data[..8].copy_from_slice(b"DCLTCCR1");
            data[8..10].copy_from_slice(&1_u16.to_le_bytes());
            data[10] = 4;
            data[16] = 1;
            data
        };

        let close = packet();
        assert!(route_selected(
            &route,
            "ClaimsProgram1111",
            &instruction(&close)
        ));
        let mut malformed = packet();
        malformed[0] = b'X';
        assert!(!route_selected(
            &route,
            "ClaimsProgram1111",
            &instruction(&malformed)
        ));
        malformed = packet();
        malformed[8] = 2;
        assert!(!route_selected(
            &route,
            "ClaimsProgram1111",
            &instruction(&malformed)
        ));
        malformed = packet();
        malformed[10] = 3;
        assert!(!route_selected(
            &route,
            "ClaimsProgram1111",
            &instruction(&malformed)
        ));
        assert!(!route_selected(
            &route,
            "DifferentClaimsProgram1111",
            &instruction(&close)
        ));
        malformed = packet();
        malformed[11] = 1;
        assert!(!route_selected(
            &route,
            "ClaimsProgram1111",
            &instruction(&malformed)
        ));
        malformed = packet();
        malformed[48] = 1;
        assert!(!route_selected(
            &route,
            "ClaimsProgram1111",
            &instruction(&malformed)
        ));
        malformed = packet();
        malformed[16..48].fill(0);
        assert!(!route_selected(
            &route,
            "ClaimsProgram1111",
            &instruction(&malformed)
        ));
        assert!(!route_selected(
            &route,
            "ClaimsProgram1111",
            &instruction(&close[..63])
        ));
    }

    #[test]
    fn claims_fractional_variants_require_exact_native_action_and_header() {
        let native = |value: u8| NativeVariantSelector {
            offset: 10,
            value,
            offset_provenance: "crates/dclutch-claims/src/fractional/request_v2.rs:23".into(),
            value_provenance: "crates/dclutch-claims/src/fractional/request_v2.rs:85".into(),
        };
        let route = |id: &str, variants: &[(&str, u8)]| Route {
            id: id.into(),
            kind: RouteKind::Action,
            parent: Some("claims/fractional_atomic_v3::process".into()),
            handler: id.split('/').nth(1).expect("handler").into(),
            provenance: "programs/dclutch-claims-sbf/src/fractional_atomic_v3.rs:1".into(),
            cfg: Vec::new(),
            selectors: variants
                .iter()
                .map(|(path, value)| Selector::Variant {
                    path: (*path).into(),
                    native: Some(native(*value)),
                })
                .collect(),
            admissible_prestates: Vec::new(),
            selected_prestates: Vec::new(),
        };
        let instruction = |data: &[u8]| CampaignInstruction {
            program_id: "ClaimsProgram1111".into(),
            data_hex: hex(data),
        };
        let packet = |action: u8| {
            let mut data = vec![0_u8; 416];
            data[..8].copy_from_slice(b"DCFREQ02");
            data[8..10].copy_from_slice(&2_u16.to_le_bytes());
            data[10] = action;
            data
        };

        let open = route(
            "claims/process_open#WholeUnwrap",
            &[
                ("FractionalExposureActionV2::Wrap", 0),
                ("FractionalExposureActionV2::WholeUnwrap", 2),
            ],
        );
        let terminal = route(
            "claims/process_terminal#TerminalZeroBurn",
            &[
                ("FractionalExposureActionV2::TerminalRedeem", 3),
                ("FractionalExposureActionV2::TerminalZeroBurn", 4),
            ],
        );
        let whole_unwrap = packet(2);
        assert!(route_selected(
            &open,
            "ClaimsProgram1111",
            &instruction(&whole_unwrap)
        ));
        assert!(!route_selected(
            &terminal,
            "ClaimsProgram1111",
            &instruction(&whole_unwrap)
        ));
        assert!(!route_selected(
            &open,
            "DifferentClaimsProgram1111",
            &instruction(&whole_unwrap)
        ));

        for hostile in [1_u8, 3, 4, u8::MAX] {
            assert!(!route_selected(
                &open,
                "ClaimsProgram1111",
                &instruction(&packet(hostile))
            ));
        }
        let mut malformed = whole_unwrap.clone();
        malformed[8] = 3;
        assert!(!route_selected(
            &open,
            "ClaimsProgram1111",
            &instruction(&malformed)
        ));
        malformed = whole_unwrap.clone();
        malformed[11] = 1;
        assert!(!route_selected(
            &open,
            "ClaimsProgram1111",
            &instruction(&malformed)
        ));
        malformed = whole_unwrap.clone();
        malformed[415] = 1;
        assert!(!route_selected(
            &open,
            "ClaimsProgram1111",
            &instruction(&malformed)
        ));
        assert!(!route_selected(
            &open,
            "ClaimsProgram1111",
            &instruction(&whole_unwrap[..415])
        ));
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
            "instructions": [{"program_id": "CoreProgram1111", "data_hex": "01"}],
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
    fn a_propagated_refusal_is_attributed_to_the_frame_that_raised_it() {
        // Core invokes a child, the child refuses 0x6, and Core unwinds by
        // returning the same code. Both frames log it. The code the
        // transaction carries is the last one; the program that RAISED it is
        // the first frame to report it, because everything after is unwinding.
        let logs = [
            "Program CoreProgram1111 invoke [1]".to_string(),
            "Program ChildProgram111 invoke [2]".to_string(),
            "Program ChildProgram111 failed: custom program error: 0x6".to_string(),
            "Program CoreProgram1111 failed: custom program error: 0x6".to_string(),
        ];
        let reported = reported_custom_code(&logs, None).expect("a reported refusal");
        assert_eq!(reported.code, 6);
        assert_eq!(reported.program.as_deref(), Some("ChildProgram111"));

        // A frame that catches its child and raises its OWN code has the last
        // word on the code, and owns it.
        let caught = [
            "Program CoreProgram1111 invoke [1]".to_string(),
            "Program ChildProgram111 invoke [2]".to_string(),
            "Program ChildProgram111 failed: custom program error: 0xa".to_string(),
            "Program CoreProgram1111 failed: custom program error: 0x3005".to_string(),
        ];
        let reported = reported_custom_code(&caught, None).expect("a reported refusal");
        assert_eq!(reported.code, 0x3005);
        assert_eq!(reported.program.as_deref(), Some("CoreProgram1111"));

        // A runtime refusal that names no program is reported honestly as
        // having none, rather than being pinned on whoever ran last.
        let unattributed = ["custom program error: 0x7".to_string()];
        let reported = reported_custom_code(&unattributed, None).expect("a reported refusal");
        assert_eq!(reported.code, 7);
        assert_eq!(reported.program, None);
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
            "instructions": [{"program_id": "CoreProgram1111", "data_hex": "01"}],
            "logs": ["Program CoreProgram1111 invoke [1]",
                     "Program CoreProgram1111 success",
                     "Program TestCaller11111 failed: custom program error: 0x6"]
        }]);

        let mut lying = executed_binding();
        lying.label = "caller refuses after Found31 committed".into();
        lying.outcome = Outcome::Refused;
        lying.refusal = Some("core/CoreSbfError::RentCredit".into());
        let report = run(&bindings(lying), &evidence(&refused));
        // This assertion used to read `admitted == 1`, with a comment saying
        // the census could not tell the collision apart from a real Core
        // refusal. It can now: the logs name TestCaller11111 as the frame that
        // raised 0x6, and the binding credits a `core/` refusal, so the claim
        // is rejected instead of recorded. Decision 0007 also makes the shared
        // number impossible going forward -- this is the belt to that braces,
        // and it is the half that would still hold if a band were misallocated.
        assert_eq!(report.admitted, 0);
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.contains("raised it, not core")),
            "{:?}",
            report.problems
        );

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
            "instructions": [{"program_id": "CoreProgram1111", "data_hex": "01"}],
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
            "instructions": [{"program_id": "CoreProgram1111", "data_hex": "01"}],
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
            "instructions": [{"program_id": "CoreProgram1111", "data_hex": "01"}],
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
