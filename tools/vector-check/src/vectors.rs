//! Decoding a manifest into the record shape of `VECTOR_SPINE_PROPOSAL.md` §3.

use std::collections::BTreeMap;

use crate::json::Value;
use crate::sha256;
use crate::taxonomy::Taxonomy;

pub const EXECUTOR_IDS: [&str; 5] = [
    "rust-reference",
    "verus-host",
    "rocq-extracted",
    "lean-checker",
    "sbf-program-test",
];

#[derive(Clone, Debug)]
pub struct Manifest {
    pub path: String,
    pub family: String,
    pub status: String,
    pub vectors: Vec<Vector>,
    /// Recomputed under DIG-2, never read from the file.
    pub computed_manifest_digest: String,
    pub declared_manifest_digest: String,
    pub declared_taxonomy_digest: Option<String>,
    pub taxonomy_version: u64,
    pub schema_version: u64,
}

#[derive(Clone, Debug)]
pub struct Vector {
    pub id: String,
    pub title: String,
    pub domain: String,
    pub surface: String,
    pub status: String,
    pub primary_property_id: String,
    pub property_ids: Vec<String>,
    pub initial_state: State,
    pub operations: Vec<Step>,
    pub final_state: Option<State>,
    pub executors: BTreeMap<String, Disposition>,
    pub comparison: Comparison,
    pub computed_digest: String,
    pub declared_digest: String,
    pub provenance_kind: String,
}

#[derive(Clone, Debug)]
pub struct State {
    pub form: String,
    pub constructed_by: String,
    pub value: Value,
}

#[derive(Clone, Debug)]
pub struct Step {
    pub step: u64,
    pub op: String,
    pub args: Value,
    pub expect: Expect,
    pub post_state: Option<State>,
}

#[derive(Clone, Debug)]
pub enum Expect {
    Ok {
        value: Option<Value>,
    },
    Refusal {
        kind: String,
        code: u32,
        name: String,
        frame: Option<String>,
    },
}

#[derive(Clone, Debug)]
pub struct Disposition {
    pub mode: String,
    /// TAX-6: the coarse code this executor is admitted to answer with.
    pub coarsens_to: Option<u32>,
    pub reason: Option<String>,
    pub blocked_by: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Comparison {
    pub byte_exact: String,
    pub byte_artifacts: usize,
    pub post_state_on_error: String,
    pub single_fault: Option<bool>,
    pub precedence_note: Option<String>,
}

const FAMILIES: [&str; 7] = [
    "kernel",
    "accumulator",
    "batch",
    "layout",
    "adapter",
    "model",
    "cross-runtime",
];
const DOMAINS: [&str; 10] = [
    "arith", "shape", "phase", "auth", "cons", "evid", "replay", "cap", "refuse", "success",
];
const SURFACES: [&str; 6] = [
    "clutch-kernel",
    "clutch-accumulator",
    "clutch-batch",
    "solana-layout",
    "solana-reference",
    "vertical-model",
];
const DISPOSITION_MODES: [&str; 5] = [
    "exact",
    "coarsened",
    "refusal-only",
    "not-applicable",
    "pending",
];
const REASONS: [&str; 6] = [
    "no-account-plane",
    "no-byte-plane",
    "no-cash-plane",
    "no-statistic-family",
    "refusal-only-evaluator",
    "spec-only-shadow",
];

fn one_of(value: &str, allowed: &[&str], what: &str) -> Result<(), String> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "{what} {value:?} is not in the closed list {allowed:?}"
        ))
    }
}

impl Manifest {
    pub fn load(path: &str, text: &str, taxonomy: &Taxonomy) -> Result<Self, String> {
        let root = crate::json::parse(text).map_err(|error| format!("{path}: {error}"))?;
        let schema = root.require("schema")?.as_str()?;
        if schema != "dragons-clutch/vector-manifest-v1" {
            return Err(format!("{path}: unknown schema {schema:?}"));
        }
        let schema_version = root.require("schema_version")?.as_small()?;
        let taxonomy_version = root.require("taxonomy_version")?.as_small()?;
        if taxonomy_version != taxonomy.version {
            return Err(format!(
                "{path}: manifest pins taxonomy_version {taxonomy_version}, the checked-out taxonomy is {} (VER-1)",
                taxonomy.version
            ));
        }
        let family = root.require("family")?.as_str()?.to_string();
        one_of(&family, &FAMILIES, "family")?;
        let status = root.require("status")?.as_str()?.to_string();
        one_of(&status, &["proposed", "frozen", "superseded"], "status")?;

        let digests = root.require("digests")?;
        let declared_manifest_digest = digests.require("manifest")?.as_str()?.to_string();
        let declared_taxonomy_digest = match digests.get("taxonomy") {
            Some(value) => Some(value.as_str()?.to_string()),
            None => None,
        };
        // DIG-2: over the JCS canonical JSON with the `digests` member removed.
        let computed_manifest_digest = sha256::hex(root.without("digests").to_jcs().as_bytes());

        let mut vectors = Vec::new();
        for item in root.require("vectors")?.as_array()? {
            vectors.push(Vector::load(item, taxonomy).map_err(|error| format!("{path}: {error}"))?);
        }
        if vectors.is_empty() {
            return Err(format!("{path}: a manifest must carry at least one vector"));
        }

        Ok(Self {
            path: path.to_string(),
            family,
            status,
            vectors,
            computed_manifest_digest,
            declared_manifest_digest,
            declared_taxonomy_digest,
            taxonomy_version,
            schema_version,
        })
    }
}

impl Vector {
    fn load(value: &Value, taxonomy: &Taxonomy) -> Result<Self, String> {
        let id = value.require("id")?.as_str()?.to_string();
        let context = |error: String| format!("vector {id}: {error}");
        let computed_digest = sha256::hex(value.without("digests").to_jcs().as_bytes());
        let declared_digest = value
            .require("digests")
            .and_then(|d| d.require("vector"))
            .and_then(|d| d.as_str())
            .map_err(&context)?
            .to_string();

        let domain = value
            .require("domain")
            .and_then(|v| v.as_str())
            .map_err(&context)?
            .to_string();
        one_of(&domain, &DOMAINS, "domain").map_err(&context)?;
        let surface = value
            .require("surface")
            .and_then(|v| v.as_str())
            .map_err(&context)?
            .to_string();
        one_of(&surface, &SURFACES, "surface").map_err(&context)?;
        let status = value
            .require("status")
            .and_then(|v| v.as_str())
            .map_err(&context)?
            .to_string();
        let title = match value.get("title") {
            Some(text) => text.as_str().map_err(&context)?.to_string(),
            None => String::new(),
        };
        let primary_property_id = value
            .require("primary_property_id")
            .and_then(|v| v.as_str())
            .map_err(&context)?
            .to_string();
        let mut property_ids = Vec::new();
        for item in value
            .require("property_ids")
            .and_then(|v| v.as_array())
            .map_err(&context)?
        {
            property_ids.push(item.as_str().map_err(&context)?.to_string());
        }
        if !property_ids.contains(&primary_property_id) {
            return Err(context(format!(
                "primary_property_id {primary_property_id} is absent from property_ids"
            )));
        }

        let provenance = value.require("provenance").map_err(&context)?;
        let provenance_kind = provenance
            .require("kind")
            .and_then(|v| v.as_str())
            .map_err(&context)?
            .to_string();
        if provenance_kind == "handwritten" {
            provenance.require("source").map_err(&context)?;
        } else if provenance_kind == "generated" {
            for key in [
                "generator",
                "generator_version",
                "seed",
                "reproduction_command",
            ] {
                provenance.require(key).map_err(&context)?;
            }
        } else {
            return Err(context(format!(
                "unknown provenance kind {provenance_kind:?}"
            )));
        }

        let initial_state =
            State::load(value.require("initial_state").map_err(&context)?).map_err(&context)?;
        let final_state = match value.get("final_state") {
            Some(state) => Some(State::load(state).map_err(&context)?),
            None => None,
        };

        let mut operations = Vec::new();
        for (index, item) in value
            .require("operations")
            .and_then(|v| v.as_array())
            .map_err(&context)?
            .iter()
            .enumerate()
        {
            let step = Step::load(item, taxonomy).map_err(&context)?;
            if step.step != index as u64 {
                return Err(context(format!(
                    "operation {index} declares step {} ; steps are dense and zero-based",
                    step.step
                )));
            }
            operations.push(step);
        }
        if operations.is_empty() {
            return Err(context("a vector must carry at least one operation".into()));
        }

        // COMP-4: all five executors carry a disposition on every vector.
        let mut executors = BTreeMap::new();
        let map = value
            .require("executors")
            .and_then(|v| v.as_object())
            .map_err(&context)?;
        for (key, entry) in map {
            if !EXECUTOR_IDS.contains(&key.as_str()) {
                return Err(context(format!("unknown executor id {key:?}")));
            }
            let mode = entry
                .require("mode")
                .and_then(|v| v.as_str())
                .map_err(&context)?
                .to_string();
            one_of(&mode, &DISPOSITION_MODES, "disposition mode").map_err(&context)?;
            let reason = match entry.get("reason") {
                Some(text) => {
                    let reason = text.as_str().map_err(&context)?.to_string();
                    one_of(&reason, &REASONS, "disposition reason").map_err(&context)?;
                    Some(reason)
                }
                None => None,
            };
            let blocked_by = match entry.get("blocked_by") {
                Some(text) => Some(text.as_str().map_err(&context)?.to_string()),
                None => None,
            };
            let coarsens_to = match entry.get("coarsens_to") {
                Some(value) => Some(
                    u32::try_from(value.as_small().map_err(&context)?)
                        .map_err(|_| context("coarsens_to out of range".into()))?,
                ),
                None => None,
            };
            if mode == "coarsened" && coarsens_to.is_none() {
                return Err(context(format!(
                    "executor {key} is coarsened with no coarsens_to (TAX-6)"
                )));
            }
            if mode != "coarsened" && coarsens_to.is_some() {
                return Err(context(format!(
                    "executor {key} declares coarsens_to under mode {mode:?}"
                )));
            }
            if mode == "not-applicable" && reason.is_none() {
                return Err(context(format!(
                    "executor {key} is not-applicable with no reason token (D2)"
                )));
            }
            if mode == "pending" && blocked_by.is_none() {
                return Err(context(format!(
                    "executor {key} is pending with no named blocker (D7)"
                )));
            }
            executors.insert(
                key.clone(),
                Disposition {
                    mode,
                    coarsens_to,
                    reason,
                    blocked_by,
                },
            );
        }
        for id in EXECUTOR_IDS {
            if !executors.contains_key(id) {
                return Err(context(format!(
                    "executor {id} carries no disposition (COMP-4)"
                )));
            }
        }

        let comparison_value = value.require("comparison").map_err(&context)?;
        if !comparison_value
            .require("semantic")
            .and_then(|v| v.as_bool())
            .map_err(&context)?
        {
            return Err(context(
                "COMP-1 makes semantic comparison unconditional".into(),
            ));
        }
        let byte_exact = comparison_value
            .require("byte_exact")
            .and_then(|v| v.as_str())
            .map_err(&context)?
            .to_string();
        one_of(
            &byte_exact,
            &["required", "optional", "not-applicable"],
            "byte_exact",
        )
        .map_err(&context)?;
        let post_state_on_error = comparison_value
            .require("post_state_on_error")
            .and_then(|v| v.as_str())
            .map_err(&context)?
            .to_string();
        one_of(
            &post_state_on_error,
            &["unchanged", "as-declared", "unspecified"],
            "post_state_on_error",
        )
        .map_err(&context)?;
        // COMP-3: an executor with no byte plane declares not-applicable, and a
        // vector that declares not-applicable may carry no byte artifact.
        let byte_artifacts = match comparison_value.get("byte_artifacts") {
            Some(list) => list.as_array().map_err(&context)?.len(),
            None => 0,
        };
        if byte_exact == "not-applicable" && byte_artifacts != 0 {
            return Err(context(
                "byte_exact is not-applicable but byte_artifacts are declared (COMP-3)".into(),
            ));
        }
        if byte_exact == "required" && byte_artifacts == 0 {
            return Err(context(
                "byte_exact is required but no byte artifact names the compared bytes (COMP-2)"
                    .into(),
            ));
        }
        let single_fault = match comparison_value.get("single_fault") {
            Some(flag) => Some(flag.as_bool().map_err(&context)?),
            None => None,
        };
        let precedence_note = match comparison_value.get("precedence_note") {
            Some(text) => Some(text.as_str().map_err(&context)?.to_string()),
            None => None,
        };
        // COMP-5: two coexisting faults must name the owning check order.
        if single_fault == Some(false) && precedence_note.is_none() {
            return Err(context(
                "single_fault is false with no precedence_note (COMP-5)".into(),
            ));
        }

        const MUTABLE_SURFACES: [&str; 3] =
            ["clutch-kernel", "clutch-accumulator", "solana-reference"];
        let refuses = operations
            .iter()
            .any(|step| matches!(step.expect, Expect::Refusal { .. }));
        if refuses
            && post_state_on_error == "unchanged"
            && MUTABLE_SURFACES.contains(&surface.as_str())
            && final_state.is_none()
            && !operations.iter().any(|step| step.post_state.is_some())
        {
            return Err(context(
                "the vector refuses on a surface that owns mutable state and declares post_state_on_error \"unchanged\", but pins no post_state or final_state, so the claim is never checked (COMP-6)"
                    .into(),
            ));
        }

        Ok(Self {
            id,
            title,
            domain,
            surface,
            status,
            primary_property_id,
            property_ids,
            initial_state,
            operations,
            final_state,
            executors,
            comparison: Comparison {
                byte_exact,
                byte_artifacts,
                post_state_on_error,
                single_fault,
                precedence_note,
            },
            computed_digest,
            declared_digest,
            provenance_kind,
        })
    }
}

impl State {
    fn load(value: &Value) -> Result<Self, String> {
        let form = value.require("form")?.as_str()?.to_string();
        let constructed_by = value.require("constructed_by")?.as_str()?.to_string();
        one_of(
            &constructed_by,
            &["constructor", "raw-fields", "operation-sequence"],
            "constructed_by",
        )?;
        Ok(Self {
            form,
            constructed_by,
            value: value.require("value")?.clone(),
        })
    }
}

impl Step {
    fn load(value: &Value, taxonomy: &Taxonomy) -> Result<Self, String> {
        let step = value.require("step")?.as_small()?;
        let op = value.require("op")?.as_str()?.to_string();
        let args = value
            .get("args")
            .cloned()
            .unwrap_or_else(|| Value::Object(BTreeMap::new()));
        let expect_value = value.require("expect")?;
        let kind = expect_value.require("result_kind")?.as_str()?;
        let expect = match kind {
            "ok" => Expect::Ok {
                value: expect_value.get("value").cloned(),
            },
            "error" | "refusal" => {
                let code = u32::try_from(expect_value.require("code")?.as_small()?)
                    .map_err(|_| "code out of range".to_string())?;
                let name = expect_value.require("name")?.as_str()?.to_string();
                let row = taxonomy.row(code)?;
                // VER-8 / TAX-2: the name and the number are one binding.
                if row.name != name {
                    return Err(format!(
                        "step {step} names code {code} as {name:?}; the taxonomy calls it {:?}",
                        row.name
                    ));
                }
                if let Some(flag) = expect_value.get("by_design") {
                    let declared = flag.as_bool()?;
                    if declared != row.by_design {
                        return Err(format!(
                            "step {step} declares by_design {declared} for {code}; the taxonomy says {}",
                            row.by_design
                        ));
                    }
                }
                let frame = match expect_value.get("frame") {
                    Some(text) => Some(text.as_str()?.to_string()),
                    None => None,
                };
                Expect::Refusal {
                    kind: kind.to_string(),
                    code,
                    name,
                    frame,
                }
            }
            other => return Err(format!("unknown result_kind {other:?}")),
        };
        let post_state = match value.get("post_state") {
            Some(state) => Some(State::load(state)?),
            None => None,
        };
        Ok(Self {
            step,
            op,
            args,
            expect,
            post_state,
        })
    }
}
