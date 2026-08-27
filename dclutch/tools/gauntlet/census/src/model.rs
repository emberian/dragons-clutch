//! Serialised shapes for the route inventory, the execution ledger, and the
//! campaign bindings the two are joined through.
//!
//! Everything here is deliberately flat and stringly-typed at the boundary: the
//! inventory is regenerated from source on every run and must survive being
//! diffed by a human who is deciding whether a route disappeared because it was
//! deleted or because the enumerator stopped recognising it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const INVENTORY_SCHEMA_V1: &str = "dclutch-gauntlet-route-inventory-v1";
pub const LEDGER_SCHEMA_V1: &str = "dclutch-gauntlet-execution-ledger-v1";

/// Where a fact came from in the source tree, as `path:line`, repo-relative.
pub type Provenance = String;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Inventory {
    pub schema: String,
    /// Repo-relative root the enumeration ran over.
    pub source_root: String,
    /// Exact source revision, when the caller supplied one.
    pub source_revision: Option<String>,
    pub programs: Vec<ProgramSurface>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProgramSurface {
    /// Cargo package name, e.g. `dclutch-core-sbf`.
    pub package: String,
    /// Short census label, e.g. `core`.
    pub label: String,
    pub crate_root: String,
    pub entrypoints: Vec<Entrypoint>,
    pub routes: Vec<Route>,
    pub refusals: Vec<Refusal>,
    /// Dispatch-position expressions the enumerator could not classify. These
    /// are printed in the report rather than dropped: an enumerator that
    /// silently under-counts is the same mirror failure one level up.
    pub unclassified: Vec<Unclassified>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Entrypoint {
    /// `entrypoint` or `entrypoint_no_alloc`.
    pub macro_name: String,
    pub function: String,
    pub provenance: Provenance,
    /// True when the enumerator resolved the function body and walked it.
    pub resolved: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RouteKind {
    /// A branch of the program's top-level instruction dispatch.
    Entry,
    /// An action tag matched inside a route's handler.
    Action,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Route {
    /// Stable identity, `<label>/<handler path>` plus any action-tag suffix.
    pub id: String,
    pub kind: RouteKind,
    pub parent: Option<String>,
    /// The function the dispatch branch hands control to.
    pub handler: String,
    /// Wire discriminants that select this branch.
    pub selectors: Vec<Selector>,
    pub provenance: Provenance,
    /// `cfg` attributes in force on the dispatch branch, verbatim.
    pub cfg: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Selector {
    /// An instruction magic compared against a leading byte span.
    Magic {
        constant: String,
        bytes: Option<String>,
        ascii: Option<String>,
        provenance: Option<Provenance>,
    },
    /// An exact-length discriminator.
    Length {
        constant: String,
        value: Option<i64>,
        provenance: Option<Provenance>,
    },
    /// A `fn is_x(instruction_data) -> bool` recogniser.
    Predicate { function: String },
    /// An enum action tag matched in a handler.
    Variant { path: String },
    /// A decoded value matched in a dispatch `match` — a width, an action
    /// byte, a named constant the pattern names directly.
    Tag { text: String },
    /// A literal compared inside the guard (an action byte, a count).
    Literal { text: String },
    /// The branch taken when no earlier guard matched.
    Fallthrough,
}

impl Selector {
    pub fn render(&self) -> String {
        match self {
            Self::Magic {
                constant,
                bytes,
                ascii,
                ..
            } => match (ascii, bytes) {
                (Some(ascii), _) => format!("magic {constant} = b\"{ascii}\""),
                (None, Some(bytes)) => format!("magic {constant} = {bytes}"),
                (None, None) => format!("magic {constant} = <unresolved>"),
            },
            Self::Length {
                constant, value, ..
            } => match value {
                Some(value) => format!("len == {constant} ({value})"),
                None => format!("len == {constant}"),
            },
            Self::Predicate { function } => format!("predicate {function}()"),
            Self::Variant { path } => format!("tag {path}"),
            Self::Tag { text } => format!("tag {text}"),
            Self::Literal { text } => format!("literal {text}"),
            Self::Fallthrough => "fallthrough (no earlier guard matched)".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Refusal {
    /// `<label>/<Enum>::<Variant>`.
    pub id: String,
    pub enum_name: String,
    pub variant: String,
    pub code: Option<i64>,
    pub doc: Option<String>,
    pub provenance: Provenance,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Unclassified {
    pub context: String,
    pub expression: String,
    pub provenance: Provenance,
    pub reason: String,
}

// ------------------------------------------------------------------- ledger

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Ledger {
    #[serde(default)]
    pub schema: String,
    #[serde(default)]
    pub observations: Vec<Observation>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// The transaction succeeded on the validator.
    Executed,
    /// The transaction refused, and the refusal was the one the binding named.
    Refused,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Observation {
    pub route: String,
    /// Which tier produced it.
    pub campaign: String,
    /// The campaign's own transaction label.
    pub label: String,
    pub signature: String,
    pub slot: u64,
    pub outcome: Outcome,
    /// The refusal the chain actually reported, when it refused.
    pub refusal: Option<String>,
    pub compute_units: Option<u64>,
    /// Program addresses the finalized log messages show as invoked. This is
    /// the chain's account of what ran, not the harness's.
    pub programs_invoked: Vec<String>,
    /// SHA-256 of the evidence document this observation was folded from.
    pub evidence_sha256: String,
    pub evidence_path: String,
}

// ----------------------------------------------------------------- bindings

/// The campaign's transaction labels, bound to census routes. Authored by the
/// gauntlet, never by the campaign: label drift must break loudly.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Bindings {
    pub campaign: String,
    /// Notes carried into the report so a reader can see why a binding exists.
    #[serde(default)]
    pub note: String,
    #[allow(clippy::struct_field_names, reason = "this IS the list of bindings")]
    pub bindings: Vec<Binding>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Binding {
    /// Exact campaign transaction label, or a `prefix*` glob.
    pub label: String,
    /// Census route ids this transaction drove.
    pub routes: Vec<String>,
    /// Program label whose invocation the chain logs must show.
    pub program: String,
    pub outcome: Outcome,
    /// Census refusal id the chain must report, when `outcome` is `refused`.
    #[serde(default)]
    pub refusal: Option<String>,
    /// Why this transaction exercises these routes.
    pub note: String,
}

// ------------------------------------------------------------------ blocked

/// Routes that cannot be driven today. Every entry names the reason and the
/// lane that owns unblocking it.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BlockedSet {
    #[serde(default)]
    pub note: String,
    pub blocked: Vec<Blocked>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Blocked {
    /// Route id, or a `prefix*` glob covering a whole program or family.
    pub route: String,
    pub reason: String,
    /// Lane or owner responsible for unblocking it.
    pub owner: String,
}

// ------------------------------------------------------------------ program

/// Chain-derived label -> program address map, read from the bootstrap plan.
pub type ProgramMap = BTreeMap<String, String>;
