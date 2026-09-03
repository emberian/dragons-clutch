//! Named admissible-prestate constants, read from the guards that use them.
//!
//! A route that reads `state.phase` answers one question -- *may this Market
//! be asked to do this now?* -- and until the guards had names the answer was
//! unreadable outside the function that computed it. `/workbench` reported
//! READY TO PREFLIGHT for acts an open Market refuses on sight for exactly
//! that reason, and the repair was not a hand mapping: it was to give the
//! guards names in the Rust that enforces them and let this enumerator read
//! them (`2b0046fb`).
//!
//! So nothing here interprets prose. A constant of type `MarketAdmissionV1` is
//! read structurally, from the initializer the compiler evaluates; a constant
//! written in a shape this module does not recognise is reported as
//! unclassified rather than guessed at, which is the same discipline the
//! dispatch classifier keeps one level up.

use std::collections::{BTreeMap, BTreeSet};

use syn::{
    Expr, Item, ItemConst, Pat,
    visit::{self, Visit},
};

use crate::{
    enumerate::{CrateIndex, at, quote_min::render_path, rust_sources},
    model::{AdmissionKind, PhaseAdmission, Prestate, Provenance, Route, Unclassified},
};

/// One persisted state machine whose admissible states a guard may declare.
///
/// A machine is named here, never inferred, because the whole content of a
/// declaration is WHICH DISCRIMINANT it is a set over. The Market's phase
/// cannot answer whether a Source may still be captured -- a Market is `Open`
/// for the whole span in which its Source moves `Primary` to `Resolved` -- so
/// a reader that lost the machine would check one set against another
/// machine's state and report an admission nobody declared.
///
/// Every machine's declaration type is deliberately the same shape: a
/// const-constructed bitset indexed by the machine's own Lean-emitted wire
/// tags. That is what lets this be ONE enumerator with a machine parameter
/// rather than one parser per state machine.
struct Machine {
    /// The declaration type this enumerator reads constants of.
    admission_type: &'static str,
    /// What routes.md and every consumer downstream calls this machine.
    label: &'static str,
    /// The enum a primary-axis variant must name.
    primary: &'static str,
    /// The constructor naming primary states alone.
    primary_constructor: &'static str,
    /// The second axis, for a machine that has one: its enum and the
    /// constructor that names exact pairs.
    secondary: Option<(&'static str, &'static str)>,
}

/// Every machine the census reads a guard's declaration for.
///
/// Adding one is a visible decision and costs a type beside its own
/// discriminant, not a case in a parser here.
const MACHINES: &[Machine] = &[
    Machine {
        admission_type: "MarketAdmissionV1",
        label: "market",
        primary: "Phase",
        primary_constructor: "phases",
        secondary: Some(("Readiness", "prestates")),
    },
    Machine {
        admission_type: "SourceAdmissionV1",
        label: "source",
        primary: "SourceResolutionPhaseV1",
        primary_constructor: "states",
        secondary: None,
    },
    Machine {
        admission_type: "DealerScenarioCheckpointAdmissionV1",
        label: "dealer-checkpoint",
        primary: "DealerScenarioCheckpointPhaseV1",
        primary_constructor: "states",
        secondary: None,
    },
    Machine {
        admission_type: "DealerScenarioReservationAdmissionV1",
        label: "dealer-reservation",
        primary: "DealerScenarioReservationStateStatusV1",
        primary_constructor: "states",
        secondary: None,
    },
    Machine {
        admission_type: "ProjectedCustodyAdmissionV1",
        label: "projected-custody",
        primary: "ProjectedCustodyPhaseV1",
        primary_constructor: "states",
        secondary: None,
    },
    Machine {
        admission_type: "DirectRootAdmissionV1",
        label: "direct-root",
        primary: "DirectRootPhaseV1",
        primary_constructor: "states",
        secondary: None,
    },
    Machine {
        admission_type: "DealerRootAdmissionV1",
        label: "dealer-root",
        primary: "DealerRootPhaseV1",
        primary_constructor: "states",
        secondary: None,
    },
];

/// Programs whose routes consult NO persisted state machine at all.
///
/// "No constant was read here" and "there is no discriminant to read" are
/// different facts, and printing them the same way tells a client to keep
/// waiting for a phase answer that will never come. Registry's eleven routes
/// are the whole of this list: they authenticate ownership, PDA derivation,
/// account vacancy and digest identity, and every one of those is a fact
/// about the ACCOUNTS in the frame rather than about a lifecycle byte
/// somebody persisted.
///
/// This is a DECLARATION, and it is carried rather than derived, because no
/// AST rule distinguishes "reads no discriminant" from "reads one this table
/// has not been told about". What keeps it from silently outliving the state
/// model is [`no_persisted_discriminant`]'s two checks: the program must
/// declare no admission constant, and its sources must name no known
/// machine's discriminant. Adding a `Phase` read to Registry makes the
/// declaration unclassified in the same run.
const NO_PERSISTED_DISCRIMINANT: &[(&str, &str)] = &[(
    "registry",
    "ownership, PDA derivation, account vacancy and digest identity; the Registry \
     persists no lifecycle discriminant for a route to consult",
)];

/// The declared reason a program has no state machine, checked not to be
/// stale.
///
/// Returns the reason, or an `Unclassified` naming the file that refutes it.
pub fn no_persisted_discriminant(
    label: &str,
    crate_src: &std::path::Path,
    root: &std::path::Path,
    declared_constants: usize,
) -> Result<Option<String>, Unclassified> {
    let Some((_, reason)) = NO_PERSISTED_DISCRIMINANT
        .iter()
        .find(|(program, _)| *program == label)
    else {
        return Ok(None);
    };
    if declared_constants > 0 {
        return Err(Unclassified {
            context: format!("{label} declares it has no state machine"),
            expression: format!("{declared_constants} admission constants"),
            provenance: crate::enumerate::relative(root, crate_src),
            reason: "a program that declares no persisted discriminant cannot also \
                     declare an admissible-state set over one"
                .into(),
        });
    }
    for path in rust_sources(crate_src).map_err(|error| Unclassified {
        context: format!("{label} declares it has no state machine"),
        expression: String::new(),
        provenance: crate::enumerate::relative(root, crate_src),
        reason: error,
    })? {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for machine in MACHINES {
            if !text.contains(&format!("{}::", machine.primary)) {
                continue;
            }
            return Err(Unclassified {
                context: format!("{label} declares it has no state machine"),
                expression: machine.primary.to_string(),
                provenance: crate::enumerate::relative(root, &path),
                reason: format!(
                    "this program reads {}, so it does consult a persisted \
                     discriminant and the declaration is stale",
                    machine.primary
                ),
            });
        }
    }
    Ok(Some((*reason).to_string()))
}

/// The machine one declaration type belongs to.
fn machine(admission_type: &str) -> Option<&'static Machine> {
    MACHINES
        .iter()
        .find(|machine| machine.admission_type == admission_type)
}

/// How many function bodies deep the scan follows a route's own calls.
///
/// Three is what the deepest real guard needs: `retire_v1::process` ->
/// `process_authenticated` -> `authenticate_market`. The descent stops at any
/// function that is itself another route's handler, so the bound is a cycle
/// guard rather than the thing that keeps one route's gates out of another's.
const MAX_GUARD_DEPTH: usize = 3;

/// One constant's declared set, with where it was written.
#[derive(Clone, Debug)]
pub struct AdmissionFact {
    pub machine: &'static str,
    pub kind: AdmissionKind,
    pub phases: Vec<String>,
    pub prestates: Vec<Prestate>,
    pub provenance: Provenance,
}

/// Workspace-wide index of admissible-prestate constants, keyed by bare name.
///
/// Bare names are enough for the same reason the constant index and the magic
/// sweep use them: these declarations are globally unique by construction. A
/// genuine collision is refused rather than guessed at.
///
/// REFUSING IS NOT ENOUGH ON ITS OWN, and this lane paid for the difference.
/// Naming Custody's handoff guard the same as Core's silently un-gated CORE's
/// route -- the count went from eleven to ten and nothing said why; it was
/// found by diffing two inventories by hand. A refusal that produces the same
/// output as an absence is not a refusal a reader can act on, so a collision
/// now emits an `Unclassified` at every colliding site, which is what the
/// report and `routes.md` show.
#[derive(Default)]
pub struct AdmissionIndex {
    facts: BTreeMap<String, Vec<AdmissionFact>>,
    /// Constants of the right type written in a shape this module cannot read.
    pub unreadable: Vec<Unclassified>,
}

impl AdmissionIndex {
    /// Every name declared more than once, with each site, as unclassified.
    pub fn collisions(&self) -> Vec<Unclassified> {
        let mut reported = Vec::new();
        for (name, facts) in &self.facts {
            if facts.len() < 2 {
                continue;
            }
            let sites: Vec<&str> = facts.iter().map(|fact| fact.provenance.as_str()).collect();
            for fact in facts {
                reported.push(Unclassified {
                    context: format!("admissible prestates {name}"),
                    expression: sites.join(", "),
                    provenance: fact.provenance.clone(),
                    reason: format!(
                        "{name} is declared {} times; a colliding name is not guessed at, so \
                         EVERY route gated by it reads as ungated until one of them is renamed",
                        facts.len()
                    ),
                });
            }
        }
        reported
    }

    pub fn resolve(&self, name: &str) -> Option<&AdmissionFact> {
        let facts = self.facts.get(name)?;
        if facts.len() == 1 {
            facts.first()
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.facts.values().filter(|facts| facts.len() == 1).count()
    }

    fn insert(&mut self, name: String, fact: AdmissionFact) {
        self.facts.entry(name).or_default().push(fact);
    }
}

/// Index every `const NAME: MarketAdmissionV1 = ...;` under `root`.
pub fn index_admissions(root: &std::path::Path) -> Result<AdmissionIndex, String> {
    let mut index = AdmissionIndex::default();
    for directory in ["crates", "programs"] {
        let base = root.join(directory);
        if !base.is_dir() {
            continue;
        }
        for path in rust_sources(&base)? {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(file) = syn::parse_file(&text) else {
                continue;
            };
            let relative = crate::enumerate::relative(root, &path);
            index_items(&file.items, &relative, &mut index);
        }
    }
    Ok(index)
}

fn index_items(items: &[Item], relative: &str, index: &mut AdmissionIndex) {
    for item in items {
        match item {
            Item::Const(constant) => index_constant(constant, relative, index),
            Item::Mod(module) => {
                if let Some((_, inner)) = &module.content {
                    index_items(inner, relative, index);
                }
            }
            Item::Impl(block) => {
                for inner in &block.items {
                    if let syn::ImplItem::Const(constant) = inner {
                        // An associated constant of the type itself -- the
                        // `NONE` sentinel -- is a declaration of nothing and
                        // is deliberately not indexed as a route's gate.
                        let _ = constant;
                    }
                }
            }
            _ => {}
        }
    }
}

fn index_constant(constant: &ItemConst, relative: &str, index: &mut AdmissionIndex) {
    let syn::Type::Path(path) = constant.ty.as_ref() else {
        return;
    };
    let Some(declared) = path.path.segments.last().map(|last| last.ident.to_string()) else {
        return;
    };
    let Some(machine) = machine(&declared) else {
        return;
    };
    let name = constant.ident.to_string();
    let provenance = at(relative, constant.ident.span());
    match read_initialiser(machine, &constant.expr) {
        Ok(fact) => index.insert(
            name,
            AdmissionFact {
                machine: machine.label,
                kind: fact.0,
                phases: fact.1,
                prestates: fact.2,
                provenance,
            },
        ),
        Err(reason) => index.unreadable.push(Unclassified {
            context: format!("admissible {} states {name}", machine.label),
            expression: render_path(&constant.expr),
            provenance,
            reason,
        }),
    }
}

type ReadSet = (AdmissionKind, Vec<String>, Vec<Prestate>);

/// Read one machine's admission initializer structurally.
fn read_initialiser(machine: &Machine, expr: &Expr) -> Result<ReadSet, String> {
    let Expr::Call(call) = strip(expr) else {
        return Err(format!(
            "initializer is not a {} constructor call",
            machine.admission_type
        ));
    };
    let Expr::Path(function) = call.func.as_ref() else {
        return Err("constructor is not a named path".into());
    };
    let constructor = function
        .path
        .segments
        .last()
        .map(|last| last.ident.to_string())
        .unwrap_or_default();
    let Some(argument) = call.args.first() else {
        return Err(format!("{constructor} was called with no set"));
    };
    let elements = array_elements(argument)
        .ok_or_else(|| format!("{constructor}'s argument is not a literal slice"))?;
    if constructor == machine.primary_constructor {
        let mut states = Vec::new();
        for element in elements {
            let state = variant(element, machine.primary)?;
            if !states.contains(&state) {
                states.push(state);
            }
        }
        return Ok((AdmissionKind::Phases, states, Vec::new()));
    }
    if let Some((secondary, constructor_name)) = machine.secondary
        && constructor == constructor_name
    {
        let mut prestates = Vec::new();
        let mut phases = Vec::new();
        for element in elements {
            let Expr::Tuple(tuple) = strip(element) else {
                return Err(format!(
                    "a prestate element is not a ({}, {secondary}) pair",
                    machine.primary
                ));
            };
            let mut parts = tuple.elems.iter();
            let (Some(phase), Some(readiness), None) = (parts.next(), parts.next(), parts.next())
            else {
                return Err("a prestate element is not a two-element tuple".into());
            };
            let phase = variant(phase, machine.primary)?;
            let readiness = variant(readiness, secondary)?;
            if !phases.contains(&phase) {
                phases.push(phase.clone());
            }
            prestates.push(Prestate { phase, readiness });
        }
        return Ok((AdmissionKind::Prestates, phases, prestates));
    }
    Err(format!(
        "unrecognised {} constructor {constructor}",
        machine.admission_type
    ))
}

fn strip(expr: &Expr) -> &Expr {
    match expr {
        Expr::Paren(inner) => strip(&inner.expr),
        Expr::Group(inner) => strip(&inner.expr),
        _ => expr,
    }
}

fn array_elements(expr: &Expr) -> Option<Vec<&Expr>> {
    let expr = match strip(expr) {
        Expr::Reference(reference) => strip(&reference.expr),
        other => other,
    };
    match expr {
        Expr::Array(array) => Some(array.elems.iter().collect()),
        _ => None,
    }
}

/// The variant name of `Enum::Variant`, checked against the enum it must name.
fn variant(expr: &Expr, enumeration: &str) -> Result<String, String> {
    let Expr::Path(path) = strip(expr) else {
        return Err(format!("expected a {enumeration} variant"));
    };
    let segments: Vec<String> = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    let (Some(name), Some(owner)) = (
        segments.last(),
        segments.get(segments.len().wrapping_sub(2)),
    ) else {
        return Err(format!("expected {enumeration}::Variant, got a bare path"));
    };
    if owner != enumeration {
        return Err(format!(
            "expected a {enumeration} variant, got {owner}::{name}"
        ));
    }
    Ok(name.clone())
}

// ------------------------------------------------------------- attribution

/// What one function body says about admissible prestates.
#[derive(Default)]
struct GuardScan {
    /// Constants named outside any variant-keyed match arm and outside any
    /// boolean branch: gates every route reaching this function passes.
    unconditional: BTreeSet<String>,
    /// `Enum::Variant => CONSTANT` arms, keyed by the variant's own name --
    /// and an or-pattern arm keyed under EVERY variant it names, because the
    /// constant gates each of them and none of their siblings. Reading such an
    /// arm as unconditional publishes `authenticate_core`'s founding-only set
    /// as the gate of the routes that admit a terminal and close a fund.
    by_variant: BTreeMap<String, BTreeSet<String>>,
    /// Call targets reached only through a variant-keyed arm, keyed the same
    /// way. A call under one arm is not a call every route through this
    /// function makes.
    calls_by_variant: BTreeMap<String, BTreeSet<String>>,
    /// The two sides of one `if`/`else`, each as what it declares and what it
    /// calls.
    ///
    /// Exactly one side runs, so a route reaching this function passes the
    /// UNION of the two sides' gates -- and passes it only if BOTH sides gate,
    /// because a side that gates nothing admits everything. `if action ==
    /// Redeem { SETTLED } else { OPEN }` is the union of two different sets;
    /// `if parent { core(OPEN) } else { basis_and_core(OPEN) }` is the union
    /// of one set with itself, which is that set, and is why the sides carry
    /// their CALLS: the gate is usually one call further down. Read as two
    /// independent gates -- which is what every entry outside a group means --
    /// a reader intersects them and reports the empty set for a route that
    /// admits three phases.
    alternatives: Vec<[BranchSide; 2]>,
    /// Straight-line call targets, for the descent.
    calls: BTreeSet<String>,
    /// Call targets reached only under a boolean condition, and the constants
    /// named there. Recorded so the under-count has a name, never attributed:
    /// `process_core_effect` calls `prepare_foundational_split` under
    /// `if foundational`, and descending into it unconditionally published
    /// `Founding` as the admissible set of the REDEEM route.
    conditional: BTreeSet<String>,
}

/// One side of an `if`: the gates written there, and the calls that may carry
/// more of them further down.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct BranchSide {
    names: BTreeSet<String>,
    calls: BTreeSet<String>,
}

struct GuardVisitor<'a> {
    index: &'a AdmissionIndex,
    /// The crate whose functions and struct fields type a method receiver.
    crate_index: &'a CrateIndex,
    scan: GuardScan,
    /// The variants whose match arm we are inside. Empty is unconditional; an
    /// or-pattern arm holds every variant it names.
    variant: BTreeSet<String>,
    /// Whether we are inside a boolean branch, whose contents gate only the
    /// executions that took it.
    conditional: bool,
    /// The type of `self` in the body being scanned, when it is a method.
    self_type: Option<String>,
    /// Every local name in scope and the type it holds, seeded from the
    /// signature. This is what makes `context.validate(false)` resolvable.
    bindings: BTreeMap<String, String>,
}

impl<'a> GuardVisitor<'a> {
    fn fresh(&self) -> GuardVisitor<'a> {
        GuardVisitor {
            index: self.index,
            crate_index: self.crate_index,
            scan: GuardScan::default(),
            variant: self.variant.clone(),
            conditional: self.conditional,
            self_type: self.self_type.clone(),
            bindings: self.bindings.clone(),
        }
    }

    /// The type one receiver expression holds, if this crate can name it.
    ///
    /// Deliberately partial. A receiver whose type is written in a dependency
    /// crate, or built by a shape not modelled here, resolves to nothing and
    /// falls through to the unique-method-name rule, which refuses on
    /// ambiguity. Guessing is what a census must not do; the under-count is
    /// already a legend entry.
    fn receiver_type(&self, expr: &Expr) -> Option<String> {
        match strip(expr) {
            Expr::Path(path) => {
                let mut segments = path.path.segments.iter();
                let (Some(first), None) = (segments.next(), segments.next()) else {
                    return None;
                };
                let name = first.ident.to_string();
                if name == "self" {
                    return self.self_type.clone();
                }
                self.bindings.get(&name).cloned()
            }
            Expr::Field(field) => {
                let owner = self.receiver_type(&field.base)?;
                let syn::Member::Named(name) = &field.member else {
                    return None;
                };
                self.crate_index
                    .field_type(&owner, &name.to_string())
                    .map(str::to_string)
            }
            Expr::MethodCall(call) => {
                let owner = self.receiver_type(&call.receiver)?;
                self.crate_index
                    .resolve_method(&owner, &call.method.to_string())?
                    .output
                    .clone()
            }
            Expr::Call(call) => {
                let Expr::Path(path) = call.func.as_ref() else {
                    return None;
                };
                self.crate_index
                    .resolve(&render_path(&path.path))?
                    .output
                    .clone()
            }
            Expr::Try(inner) => self.receiver_type(&inner.expr),
            Expr::Reference(inner) => self.receiver_type(&inner.expr),
            Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
                self.receiver_type(&unary.expr)
            }
            Expr::Struct(structure) => structure
                .path
                .segments
                .last()
                .map(|last| last.ident.to_string()),
            _ => None,
        }
    }

    /// Bind each name of a destructuring `let` to its own element type.
    ///
    /// `let (checkpoint, checkpoint_prestate_digest) = read_checkpoint(..)?`
    /// is how Trading's reservation body gets the checkpoint whose two
    /// `append_*` methods hold the gate, and a tuple pattern named nothing
    /// until now. Arity must agree exactly: a signature this file reads as
    /// two elements against a pattern of three is a disagreement, and binding
    /// the prefix would type one name with its neighbour's type.
    fn bind_tuple(&mut self, tuple: &syn::PatTuple, initializer: Option<&Expr>) {
        let Some(initializer) = initializer else {
            return;
        };
        let Some(elements) = self.initializer_elements(initializer) else {
            return;
        };
        if elements.len() != tuple.elems.len() {
            return;
        }
        for (pattern, declared) in tuple.elems.iter().zip(elements) {
            let Pat::Ident(ident) = pattern else {
                continue;
            };
            let Some(declared) = declared else {
                continue;
            };
            self.bindings.insert(ident.ident.to_string(), declared);
        }
    }

    /// The element types an initializer expression yields, when it is a call
    /// to a function this crate index can name and that function returns a
    /// tuple.
    fn initializer_elements(&self, expr: &Expr) -> Option<Vec<Option<String>>> {
        let elements = match strip(expr) {
            Expr::Try(inner) => return self.initializer_elements(&inner.expr),
            Expr::Call(call) => {
                let Expr::Path(path) = call.func.as_ref() else {
                    return None;
                };
                &self
                    .crate_index
                    .resolve(&render_path(&path.path))?
                    .output_elements
            }
            Expr::MethodCall(call) => {
                let owner = self.receiver_type(&call.receiver)?;
                &self
                    .crate_index
                    .resolve_method(&owner, &call.method.to_string())?
                    .output_elements
            }
            _ => return None,
        };
        if elements.is_empty() {
            return None;
        }
        Some(elements.clone())
    }

    /// How a call this scan resolved is spelled for the descent.
    fn method_target(&self, call: &syn::ExprMethodCall) -> Option<String> {
        let name = call.method.to_string();
        if let Some(owner) = self.receiver_type(&call.receiver)
            && self.crate_index.resolve_method(&owner, &name).is_some()
        {
            return Some(format!("{owner}::{name}"));
        }
        let sole = self.crate_index.sole_method(&name)?;
        let owner = sole.self_type.clone()?;
        Some(format!("{owner}::{name}"))
    }

    /// Record one resolved call target under whatever condition encloses it.
    fn note_call(&mut self, target: String) {
        if self.conditional {
            self.scan.conditional.insert(target);
        } else if self.variant.is_empty() {
            self.scan.calls.insert(target);
        } else {
            for variant in &self.variant {
                self.scan
                    .calls_by_variant
                    .entry(variant.clone())
                    .or_default()
                    .insert(target.clone());
            }
        }
    }

    fn note(&mut self, name: &str) {
        if self.index.resolve(name).is_none() {
            return;
        }
        if self.conditional {
            self.scan.conditional.insert(name.to_string());
            return;
        }
        if self.variant.is_empty() {
            self.scan.unconditional.insert(name.to_string());
            return;
        }
        for variant in &self.variant {
            self.scan
                .by_variant
                .entry(variant.clone())
                .or_default()
                .insert(name.to_string());
        }
    }

    /// Absorb a sub-scan made under a condition: nothing it saw is a gate every
    /// route through this function passes.
    fn absorb_conditional(&mut self, scan: GuardScan) {
        self.scan.alternatives.extend(scan.alternatives);
        self.scan.conditional.extend(scan.unconditional);
        self.scan.conditional.extend(scan.conditional);
        self.scan.conditional.extend(scan.calls);
        for names in scan.by_variant.into_values() {
            self.scan.conditional.extend(names);
        }
        for calls in scan.calls_by_variant.into_values() {
            self.scan.conditional.extend(calls);
        }
    }

    /// What one branch declares and calls, scanned in isolation.
    fn side(&self, scan_one: impl FnOnce(&mut GuardVisitor<'a>)) -> (BranchSide, GuardScan) {
        let mut inner = self.fresh();
        scan_one(&mut inner);
        let side = BranchSide {
            names: inner
                .scan
                .unconditional
                .iter()
                .cloned()
                .chain(inner.scan.by_variant.values().flatten().cloned())
                .collect(),
            calls: inner
                .scan
                .calls
                .iter()
                .cloned()
                .chain(inner.scan.calls_by_variant.values().flatten().cloned())
                .collect(),
        };
        (side, inner.scan)
    }
}

impl<'ast> Visit<'ast> for GuardVisitor<'_> {
    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if let Some(last) = node.path.segments.last() {
            self.note(&last.ident.to_string());
        }
        visit::visit_expr_path(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let Expr::Path(path) = node.func.as_ref() {
            self.note_call(render_path(&path.path));
        }
        visit::visit_expr_call(self, node);
    }

    /// A method call is a call, and for six Trading routes it is THE call.
    ///
    /// `input.context.validate(false)?` runs a body holding two phase gates,
    /// and until this existed the descent saw a path-call enumerator walk past
    /// it. The receiver's type is resolved first and the method looked up under
    /// it; a receiver this crate cannot type falls back to the sole method of
    /// that name, and a name carried by two types resolves to nothing rather
    /// than to a guess.
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if let Some(target) = self.method_target(node) {
            self.note_call(target);
        }
        visit::visit_expr_method_call(self, node);
    }

    /// A `let` names a type, and a receiver is usually a local.
    fn visit_local(&mut self, node: &'ast syn::Local) {
        visit::visit_local(self, node);
        if let Pat::Tuple(tuple) = &node.pat {
            self.bind_tuple(tuple, node.init.as_ref().map(|init| init.expr.as_ref()));
            return;
        }
        let (name, declared) = match &node.pat {
            Pat::Type(typed) => {
                let Pat::Ident(ident) = typed.pat.as_ref() else {
                    return;
                };
                (
                    ident.ident.to_string(),
                    crate::enumerate::declared_type_name(&typed.ty),
                )
            }
            Pat::Ident(ident) => {
                let Some(init) = &node.init else {
                    return;
                };
                (ident.ident.to_string(), self.receiver_type(&init.expr))
            }
            _ => return,
        };
        if let Some(declared) = declared {
            self.bindings.insert(name, declared);
        }
    }

    /// A match keys each arm by its variant, and a TWO-arm one is also a
    /// selection.
    ///
    /// Variant keying is the exact answer and stays the preferred one: a route
    /// whose id names `AdmitTerminal` reads that arm's set and no sibling's.
    /// But a route id names a variant only when the DISPATCH named it, and a
    /// guard is often keyed by an argument the dispatch never saw. Trading's
    /// reserve and rollback routes are two `pub fn`s that forward to one body
    /// with `DealerScenarioReservationActionV1::Reserve` and `::Rollback`, and
    /// the body's `match expected_action` has one arm per act. Neither route
    /// id carries a `#` tag -- each IS its own route, selected by its own
    /// instruction magic -- so both read as ungated while the two constants
    /// that gate them sat one method call inside the arms.
    ///
    /// With exactly two arms the construct is an `if`/`else` written the other
    /// way: one side runs, so the route passes the UNION, recorded as an
    /// alternative pair and resolved after the descent exactly as a selection
    /// is. Precedence needs no rule of its own -- a route that DOES name one
    /// of the two variants picks that arm up as an unconditional gate, and
    /// `for_route` already drops any group holding a name that also stands
    /// alone. Three arms or more stay variant-keyed only: the pair carries two
    /// sides, and a wider union is a claim this shape cannot make honestly.
    fn visit_expr_match(&mut self, node: &'ast syn::ExprMatch) {
        self.visit_expr(&node.expr);
        for arm in &node.arms {
            let outer = core::mem::take(&mut self.variant);
            let named = pattern_variants(&arm.pat);
            self.variant = if named.is_empty() {
                outer.clone()
            } else {
                named
            };
            if let Some(guard) = &arm.guard {
                self.visit_expr(&guard.1);
            }
            self.visit_expr(&arm.body);
            self.variant = outer;
        }
        let [first, second] = &node.arms[..] else {
            return;
        };
        let (first_body, second_body) = (first.body.clone(), second.body.clone());
        let (taken, _) = self.side(|inner| inner.visit_expr(&first_body));
        let (untaken, _) = self.side(|inner| inner.visit_expr(&second_body));
        // A side that gates nothing admits everything, so the pair is no gate
        // at all -- the same rule the two sides of an `if` are held to. The
        // sub-scans are discarded rather than absorbed: the arms were already
        // walked above, and every nested alternative they hold is already in
        // `self.scan`.
        if (taken.names.is_empty() && taken.calls.is_empty())
            || (untaken.names.is_empty() && untaken.calls.is_empty())
        {
            return;
        }
        self.scan.alternatives.push([taken, untaken]);
    }

    /// A `for` body may run zero times, so what it gates is not every
    /// route's gate.
    ///
    /// The same defect as the one-sided `if`, one construct over, and it was
    /// live: Trading's commit route authenticates each locked reservation
    /// inside `for ordinal in 0..context.effect_count`, and a scenario whose
    /// evaluation selected NO Custody effect commits with that loop never
    /// entered -- the codec seals such a checkpoint straight to `Reserved`.
    /// Attributing the `Active` reservation set to the route would have told
    /// a client the commit needs a live escrow that does not exist.
    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        self.visit_expr(&node.expr);
        let body = node.body.clone();
        let (_, scan) = self.side(|inner| inner.visit_block(&body));
        self.absorb_conditional(scan);
    }

    /// A `while` body may run zero times, for the same reason.
    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.visit_expr(&node.cond);
        let body = node.body.clone();
        let (_, scan) = self.side(|inner| inner.visit_block(&body));
        self.absorb_conditional(scan);
    }

    /// An `if` is a boolean selection, and the census cannot evaluate it.
    ///
    /// So it reads the two sides instead. With an `else`, exactly one side
    /// runs and the route passes the union of the two -- recorded as an
    /// alternative pair and resolved after the descent, because a side's gate
    /// is usually one call further down. Without an `else`, or with a side
    /// that gates nothing, the branch gates only the executions that took it:
    /// what it says is recorded as conditional and NEVER attributed. That is
    /// the under-count the `phase` column's legend already names, and it
    /// stands in place of a false claim -- `process_core_effect` calls
    /// `prepare_foundational_split` under `if foundational`, and descending
    /// into it unconditionally published `Founding` as the admissible set of
    /// the REDEEM route.
    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        self.visit_expr(&node.cond);
        let then_branch = node.then_branch.clone();
        let (taken, taken_scan) = self.side(|inner| inner.visit_block(&then_branch));
        let Some((_, otherwise)) = &node.else_branch else {
            self.absorb_conditional(taken_scan);
            return;
        };
        let (untaken, untaken_scan) = self.side(|inner| inner.visit_expr(otherwise));
        let taken_gates = !taken.names.is_empty() || !taken.calls.is_empty();
        let untaken_gates = !untaken.names.is_empty() || !untaken.calls.is_empty();
        if taken_gates && untaken_gates {
            self.scan.alternatives.extend(taken_scan.alternatives);
            self.scan.alternatives.extend(untaken_scan.alternatives);
            self.scan.alternatives.push([taken, untaken]);
            return;
        }
        self.absorb_conditional(taken_scan);
        self.absorb_conditional(untaken_scan);
    }
}

/// Every variant name an arm pattern selects.
///
/// An or-pattern selects several, and a constant under it gates each of them
/// -- and none of the sibling arms. Reading it as naming NOTHING made it
/// unconditional, so `authenticate_core`'s
/// `CreateFund | VerifyFundReady => ..` arm published a founding-only set as
/// the gate of the two sibling routes that admit a terminal and close a fund.
/// An empty result is the honest "this arm names no variant", which inherits
/// whatever match encloses it.
fn pattern_variants(pattern: &Pat) -> BTreeSet<String> {
    match pattern {
        Pat::Path(path) => path
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .into_iter()
            .collect(),
        Pat::TupleStruct(tuple) => tuple
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .into_iter()
            .collect(),
        Pat::Struct(structure) => structure
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .into_iter()
            .collect(),
        Pat::Ident(ident) => ident
            .subpat
            .as_ref()
            .map(|(_, inner)| pattern_variants(inner))
            .unwrap_or_default(),
        Pat::Or(alternatives) => alternatives
            .cases
            .iter()
            .flat_map(pattern_variants)
            .collect(),
        Pat::Reference(inner) => pattern_variants(&inner.pat),
        Pat::Paren(inner) => pattern_variants(&inner.pat),
        _ => BTreeSet::new(),
    }
}

/// Read the admissible-prestate gates each route in one program passes.
pub struct GuardMap<'a> {
    index: &'a AdmissionIndex,
    crate_index: &'a CrateIndex,
    /// Every route handler in the program: the descent stops at these, so one
    /// route's gates never leak into the entry route that dispatches to it.
    boundaries: BTreeSet<String>,
    /// Each route's handler and parent, so an action route can start its scan
    /// at every ancestor as well as at itself.
    ///
    /// The boundary rule stops one route's gates leaking UP into the entry
    /// route that dispatches to it; the parent chain is the other direction,
    /// and it is not symmetric. Resolution authenticates the Market in
    /// `process_core_effect` -- one `match request.action` with four arms --
    /// and only then dispatches to `process_create`, `process_verify`,
    /// `process_admit` and `process_close`. The gate each of those four passes
    /// is written in their parent, keyed by their own action, and a scan that
    /// started at the child alone reported all four as ungated.
    lineage: BTreeMap<String, (String, Option<String>)>,
    /// Keyed by `module::function`, the identity a call is resolved from.
    scans: BTreeMap<String, GuardScan>,
}

impl<'a> GuardMap<'a> {
    pub fn new(index: &'a AdmissionIndex, crate_index: &'a CrateIndex, routes: &[Route]) -> Self {
        let boundaries = routes
            .iter()
            .map(|route| handler_path(route).to_string())
            .collect();
        let lineage = routes
            .iter()
            .map(|route| {
                (
                    route.id.clone(),
                    (handler_path(route).to_string(), route.parent.clone()),
                )
            })
            .collect();
        Self {
            index,
            crate_index,
            boundaries,
            lineage,
            scans: BTreeMap::new(),
        }
    }

    /// One function body's scan, with the identity it is cached under and the
    /// module its own calls resolve from.
    ///
    /// Two types in one module may each carry a `validate`, so a method's
    /// identity carries the type it is written on; the MODULE is returned
    /// separately because that -- not the identity -- is what resolves the
    /// calls the body makes.
    fn scan(&mut self, module: &str, path: &str) -> Option<(String, String, &GuardScan)> {
        let crate_index = self.crate_index;
        let function = crate_index.resolve_from(module, path)?;
        let key = match &function.self_type {
            Some(owner) => format!("{}::{owner}::{}", function.module, function.name),
            None => format!("{}::{}", function.module, function.name),
        };
        let here = function.module.clone();
        if !self.scans.contains_key(&key) {
            let mut visitor = GuardVisitor {
                index: self.index,
                crate_index,
                scan: GuardScan::default(),
                variant: BTreeSet::new(),
                conditional: false,
                self_type: function.self_type.clone(),
                bindings: function.inputs.iter().cloned().collect(),
            };
            visitor.visit_block(&function.block);
            self.scans.insert(key.clone(), visitor.scan);
        }
        let scan = self.scans.get(&key)?;
        Some((key, here, scan))
    }

    /// Every gate one side of an `if` reaches, following its calls under the
    /// same depth bound and the same route-handler boundary as the main
    /// descent.
    fn resolve_side(
        &mut self,
        module: &str,
        side: &BranchSide,
        depth: usize,
        selected: &Option<String>,
    ) -> BTreeSet<String> {
        let mut names = side.names.clone();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut frontier: Vec<(String, String, usize)> = side
            .calls
            .iter()
            .map(|call| (module.to_string(), call.clone(), depth + 1))
            .collect();
        while let Some((module, path, depth)) = frontier.pop() {
            if depth > MAX_GUARD_DEPTH {
                continue;
            }
            if self.boundaries.contains(&path) {
                continue;
            }
            let Some((key, here, scan)) = self.scan(&module, &path) else {
                continue;
            };
            if !seen.insert(key.clone()) {
                continue;
            }
            names.extend(scan.unconditional.iter().cloned());
            if let Some(selected) = selected
                && let Some(keyed) = scan.by_variant.get(selected)
            {
                names.extend(keyed.iter().cloned());
            }
            let mut calls: Vec<String> = scan.calls.iter().cloned().collect();
            if let Some(selected) = selected
                && let Some(keyed) = scan.calls_by_variant.get(selected)
            {
                calls.extend(keyed.iter().cloned());
            }
            for call in calls {
                frontier.push((here.clone(), call, depth + 1));
            }
        }
        names
    }

    /// This route's handler, then each ancestor's, outermost last.
    ///
    /// A cycle in the recorded parents would loop forever, so the walk is
    /// bounded by the number of routes and stops the first time it revisits an
    /// id -- an enumerator that hangs is worse than one that under-reports.
    fn ancestry(&self, route: &Route) -> Vec<String> {
        let mut handlers = vec![handler_path(route).to_string()];
        let mut seen: BTreeSet<String> = [route.id.clone()].into_iter().collect();
        let mut parent = route.parent.clone();
        while let Some(id) = parent {
            if !seen.insert(id.clone()) {
                break;
            }
            let Some((handler, next)) = self.lineage.get(&id) else {
                break;
            };
            if !handlers.contains(handler) {
                handlers.push(handler.clone());
            }
            parent = next.clone();
        }
        handlers
    }

    /// The gates `route` passes, as declared constants.
    pub fn for_route(&mut self, route: &Route) -> Vec<PhaseAdmission> {
        let selected = route_variant(route);
        let mut names: BTreeSet<String> = BTreeSet::new();
        let mut groups: Vec<BTreeSet<String>> = Vec::new();
        let mut pending: Vec<(String, [BranchSide; 2], usize)> = Vec::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        // The route's own handler and every ancestor's, all at depth 0: a
        // parent's gates are not one call away from the child, they are on the
        // same execution before it.
        let mut frontier: Vec<(String, String, usize)> = self
            .ancestry(route)
            .into_iter()
            .map(|handler| (String::new(), handler, 0usize))
            .collect();
        while let Some((module, path, depth)) = frontier.pop() {
            if depth > MAX_GUARD_DEPTH {
                continue;
            }
            // Another route's handler owns its own gates.
            if depth > 0 && self.boundaries.contains(&path) {
                continue;
            }
            let Some((key, here, scan)) = self.scan(&module, &path) else {
                continue;
            };
            if !seen.insert(key.clone()) {
                continue;
            }
            names.extend(scan.unconditional.iter().cloned());
            if let Some(selected) = &selected
                && let Some(keyed) = scan.by_variant.get(selected)
            {
                names.extend(keyed.iter().cloned());
            }
            for pair in scan.alternatives.clone() {
                pending.push((here.clone(), pair, depth));
            }
            let mut calls: Vec<String> = scan.calls.iter().cloned().collect();
            if let Some(selected) = &selected
                && let Some(keyed) = scan.calls_by_variant.get(selected)
            {
                calls.extend(keyed.iter().cloned());
            }
            for call in calls {
                frontier.push((here.clone(), call, depth + 1));
            }
        }
        // A side that gates nothing admits everything, so a pair with one such
        // side is no gate at all. A pair whose sides resolve to the same set
        // is that set, written twice.
        for (module, [taken, untaken], depth) in pending {
            let left = self.resolve_side(&module, &taken, depth, &selected);
            if left.is_empty() {
                continue;
            }
            let right = self.resolve_side(&module, &untaken, depth, &selected);
            if right.is_empty() {
                continue;
            }
            let group: BTreeSet<String> = left.union(&right).cloned().collect();
            if !groups.contains(&group) {
                groups.push(group);
            }
        }
        // A constant that also stands alone as an unconditional gate is not an
        // alternative: the conjunct it is written in runs on every execution.
        groups.retain(|group| group.iter().all(|name| !names.contains(name)));
        let mut admissions: Vec<PhaseAdmission> = Vec::new();
        let mut push = |name: String, alternative: Option<u32>, index: &AdmissionIndex| {
            if let Some(fact) = index.resolve(&name) {
                admissions.push(PhaseAdmission {
                    constant: name,
                    machine: fact.machine.to_string(),
                    kind: fact.kind,
                    phases: fact.phases.clone(),
                    prestates: fact.prestates.clone(),
                    provenance: fact.provenance.clone(),
                    alternative,
                });
            }
        };
        for name in names {
            push(name, None, self.index);
        }
        for (group, members) in groups.iter().enumerate() {
            let group = u32::try_from(group).unwrap_or(u32::MAX);
            for name in members {
                push(name.clone(), Some(group), self.index);
            }
        }
        admissions.sort_by(|left, right| {
            (left.alternative, &left.machine, &left.constant).cmp(&(
                right.alternative,
                &right.machine,
                &right.constant,
            ))
        });
        admissions
    }
}

/// A route's handler as a resolvable function path.
fn handler_path(route: &Route) -> &str {
    match route.handler.find(" (inline") {
        Some(index) => &route.handler[..index],
        None => &route.handler,
    }
}

/// The single variant a route's id names, if it names one.
///
/// The id's `#tag` suffix is the MOST SPECIFIC discriminant the dispatch walk
/// found, which is exactly the granularity a variant-keyed guard is written
/// at. An entry route selected by three actions has a tag naming one of them
/// only when the dispatch does, so a per-action gate is never attributed to
/// the union route that reaches all three.
fn route_variant(route: &Route) -> Option<String> {
    let tag = route.id.rsplit_once('#')?.1;
    if tag.is_empty() || tag.contains(',') || tag.contains('(') {
        return None;
    }
    Some(tag.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn constant(source: &str) -> Result<ReadSet, String> {
        machine_constant("MarketAdmissionV1", source)
    }

    /// Read one constant as the named machine's declaration type.
    fn machine_constant(admission_type: &str, source: &str) -> Result<ReadSet, String> {
        let file = syn::parse_file(source).expect("parses");
        let Item::Const(item) = &file.items[0] else {
            panic!("not a const")
        };
        read_initialiser(
            machine(admission_type).expect("a known machine"),
            &item.expr,
        )
    }

    #[test]
    fn exact_prestates_carry_their_pairs_and_their_projection() {
        let (kind, phases, prestates) = constant(
            "const A: MarketAdmissionV1 = MarketAdmissionV1::prestates(&[
                 (Phase::Founding, Readiness::Prepaid),
                 (Phase::Founding, Readiness::Ready),
                 (Phase::Open, Readiness::Consumed),
             ]);",
        )
        .expect("readable");
        assert_eq!(kind, AdmissionKind::Prestates);
        assert_eq!(phases, ["Founding", "Open"]);
        assert_eq!(prestates.len(), 3);
        assert_eq!(prestates[1].phase, "Founding");
        assert_eq!(prestates[1].readiness, "Ready");
    }

    #[test]
    fn a_phase_only_constant_carries_no_pairs() {
        let (kind, phases, prestates) =
            constant("const A: MarketAdmissionV1 = MarketAdmissionV1::phases(&[Phase::Retiring]);")
                .expect("readable");
        assert_eq!(kind, AdmissionKind::Phases);
        assert_eq!(phases, ["Retiring"]);
        assert!(prestates.is_empty());
    }

    /// A second machine is read by the same enumerator, against its own enum.
    ///
    /// The check that the machine table is a parameter and not a second parser:
    /// `SourceAdmissionV1::states(&[SourceResolutionPhaseV1::Primary])` reads,
    /// and the SAME text as a `MarketAdmissionV1` does not, because a `Phase`
    /// is what that machine's variants must name.
    #[test]
    fn each_machine_is_read_against_its_own_discriminant() {
        let (kind, states, prestates) = machine_constant(
            "SourceAdmissionV1",
            "const A: SourceAdmissionV1 = \
             SourceAdmissionV1::states(&[SourceResolutionPhaseV1::Primary]);",
        )
        .expect("readable");
        assert_eq!(kind, AdmissionKind::Phases);
        assert_eq!(states, ["Primary"]);
        assert!(prestates.is_empty());

        assert!(
            machine_constant(
                "MarketAdmissionV1",
                "const A: MarketAdmissionV1 = \
                 MarketAdmissionV1::phases(&[SourceResolutionPhaseV1::Primary]);",
            )
            .is_err(),
            "a Source state passed as a Market phase must refuse"
        );
        assert!(
            machine_constant(
                "SourceAdmissionV1",
                "const A: SourceAdmissionV1 = SourceAdmissionV1::prestates(&[(A::B, C::D)]);",
            )
            .is_err(),
            "a one-axis machine has no pair constructor"
        );
    }

    /// The whole point of reading the constant structurally: a shape this
    /// module does not understand must REFUSE, so the route shows as ungated
    /// rather than taking a set nobody wrote.
    #[test]
    fn an_unreadable_shape_is_refused_rather_than_guessed() {
        assert!(constant("const A: MarketAdmissionV1 = OTHER;").is_err());
        assert!(
            constant("const A: MarketAdmissionV1 = MarketAdmissionV1::prestates(SOME_SLICE);")
                .is_err()
        );
        assert!(
            constant("const A: MarketAdmissionV1 = MarketAdmissionV1::phases(&[Other::Thing]);")
                .is_err()
        );
        assert!(
            constant("const A: MarketAdmissionV1 = MarketAdmissionV1::everything(&[]);").is_err()
        );
    }

    /// A collision must be REPORTED, not merely refused.
    ///
    /// The two are easy to conflate and the difference cost this lane real
    /// time: refusing a colliding name produces exactly the output an absent
    /// name produces, so naming Custody's handoff guard the same as Core's
    /// un-gated Core's route and the only symptom was a count.
    #[test]
    fn a_colliding_name_is_refused_and_said_out_loud() {
        let fact = |where_: &str| AdmissionFact {
            machine: "market",
            kind: AdmissionKind::Phases,
            phases: vec!["Retiring".into()],
            prestates: Vec::new(),
            provenance: where_.into(),
        };
        let mut index = AdmissionIndex::default();
        index.insert("ALONE_V1".into(), fact("programs/a/src/one.rs:1"));
        assert!(index.resolve("ALONE_V1").is_some());
        assert!(index.collisions().is_empty());
        assert_eq!(index.len(), 1);

        index.insert("SHARED_V1".into(), fact("programs/a/src/two.rs:2"));
        index.insert("SHARED_V1".into(), fact("programs/b/src/two.rs:3"));
        assert!(
            index.resolve("SHARED_V1").is_none(),
            "a collision is not guessed at"
        );
        assert_eq!(
            index.len(),
            1,
            "a colliding name counts as no readable constant"
        );
        let collisions = index.collisions();
        assert_eq!(
            collisions.len(),
            2,
            "every colliding site is reported, not just one"
        );
        for entry in &collisions {
            assert!(entry.reason.contains("declared 2 times"));
            assert!(entry.expression.contains("programs/a/src/two.rs:2"));
            assert!(entry.expression.contains("programs/b/src/two.rs:3"));
        }
        assert!(
            collisions
                .iter()
                .any(|e| e.provenance.starts_with("programs/a/"))
        );
        assert!(
            collisions
                .iter()
                .any(|e| e.provenance.starts_with("programs/b/"))
        );
    }

    /// Scan one function body against an index holding exactly `declared`.
    fn scan_body(source: &str, declared: &[&str]) -> GuardScan {
        let mut index = AdmissionIndex::default();
        for name in declared {
            index.insert(
                (*name).to_string(),
                AdmissionFact {
                    machine: "market",
                    kind: AdmissionKind::Phases,
                    phases: vec!["Open".into()],
                    prestates: Vec::new(),
                    provenance: "programs/a/src/one.rs:1".into(),
                },
            );
        }
        let crate_index = crate::enumerate::index_source("m", source);
        let file = syn::parse_file(source).expect("parses");
        let Item::Fn(function) = &file.items[0] else {
            panic!("not a fn")
        };
        let mut visitor = GuardVisitor {
            index: &index,
            crate_index: &crate_index,
            scan: GuardScan::default(),
            variant: BTreeSet::new(),
            conditional: false,
            self_type: None,
            bindings: crate::enumerate::signature_inputs(&function.sig)
                .into_iter()
                .collect(),
        };
        visitor.visit_block(&function.block);
        visitor.scan
    }

    /// A guard reached only when a boolean branch is taken gates only the
    /// executions that took it.
    ///
    /// This is the exact shape that published `Founding` as the admissible set
    /// of Claims' REDEEM route: `process_core_effect` calls
    /// `prepare_foundational_split` under `if foundational`, and a descent
    /// that walked into it regardless attributed a founding-only gate to three
    /// routes that never call it. An under-count is a legend entry; this was a
    /// false claim, and a client acting on it refuses a valid act.
    #[test]
    fn a_gate_behind_a_one_sided_condition_is_not_every_routes_gate() {
        let scan = scan_body(
            "fn process() { if foundational { split(A_V1); } authenticate(); }",
            &["A_V1"],
        );
        assert!(scan.unconditional.is_empty(), "{:?}", scan.unconditional);
        assert!(!scan.calls.contains("split"), "{:?}", scan.calls);
        assert!(scan.calls.contains("authenticate"));
        assert!(scan.conditional.contains("A_V1"));
        assert!(scan.conditional.contains("split"));
        assert!(scan.alternatives.is_empty());
    }

    /// `if c { A } else { B }` is ONE gate whose set is `A | B`.
    #[test]
    fn the_two_sides_of_a_selection_are_one_gate_and_not_two() {
        let scan = scan_body(
            "fn process() { let set = if redeeming { A_V1 } else { B_V1 }; check(set); }",
            &["A_V1", "B_V1"],
        );
        assert!(scan.unconditional.is_empty());
        assert_eq!(scan.alternatives.len(), 1);
        let [taken, untaken] = &scan.alternatives[0];
        assert_eq!(taken.names, ["A_V1".to_string()].into_iter().collect());
        assert_eq!(untaken.names, ["B_V1".to_string()].into_iter().collect());
    }

    /// A side carries its CALLS, because the gate is usually one call down.
    ///
    /// `if parent { core(OPEN) } else { basis_and_core() }` gates on the same
    /// set either way, and a rule that read only the constants written at the
    /// site would see one side declaring nothing and drop the gate.
    #[test]
    fn a_side_that_only_calls_still_counts_as_gating() {
        let scan = scan_body(
            "fn process() { let caps = if parent { core(A_V1) } else { basis() }; }",
            &["A_V1"],
        );
        assert_eq!(scan.alternatives.len(), 1);
        let [taken, untaken] = &scan.alternatives[0];
        assert_eq!(taken.names, ["A_V1".to_string()].into_iter().collect());
        assert!(untaken.names.is_empty());
        assert!(untaken.calls.contains("basis"));
    }

    /// An or-pattern arm gates every variant it names, and no sibling arm's.
    ///
    /// Resolution's `authenticate_core` matches `CreateFund | VerifyFundReady`
    /// against two sibling arms for `AdmitTerminal` and `CloseFund`. Read as
    /// naming no variant, the arm was unconditional and published a
    /// founding-only set as the gate of the two routes that admit a terminal
    /// and close a fund -- neither of which the arm can be reached by.
    #[test]
    fn an_or_pattern_arm_keys_every_variant_it_names_and_no_others() {
        let scan = scan_body(
            "fn process() { match action { A::Create | A::Verify => gate(A_V1),              A::Admit => gate(B_V1), A::Close => gate(C_V1) } }",
            &["A_V1", "B_V1", "C_V1"],
        );
        assert!(scan.unconditional.is_empty(), "{:?}", scan.unconditional);
        assert_eq!(
            scan.by_variant.get("Create"),
            Some(&["A_V1".to_string()].into_iter().collect())
        );
        assert_eq!(
            scan.by_variant.get("Verify"),
            Some(&["A_V1".to_string()].into_iter().collect())
        );
        assert_eq!(
            scan.by_variant.get("Admit"),
            Some(&["B_V1".to_string()].into_iter().collect())
        );
        assert_eq!(
            scan.by_variant.get("Close"),
            Some(&["C_V1".to_string()].into_iter().collect())
        );
    }

    /// A two-arm `match` is a selection, and its two arms unite.
    ///
    /// Trading's reserve and rollback routes forward into one body whose
    /// `match expected_action` calls `append_reservation` in one arm and
    /// `append_rollback` in the other. Neither route id names a variant, so
    /// variant keying alone answered nothing and both read as ungated.
    #[test]
    fn a_two_arm_match_unites_the_way_a_selection_does() {
        let scan = scan_body(
            "fn process(action: A) { let next = match action { A::Reserve => gate(A_V1), A::Rollback => gate(B_V1) }; }",
            &["A_V1", "B_V1"],
        );
        assert!(scan.unconditional.is_empty(), "{:?}", scan.unconditional);
        assert_eq!(
            scan.by_variant.get("Reserve"),
            Some(&["A_V1".to_string()].into_iter().collect())
        );
        assert_eq!(scan.alternatives.len(), 1);
        let [taken, untaken] = &scan.alternatives[0];
        assert_eq!(taken.names, ["A_V1".to_string()].into_iter().collect());
        assert_eq!(untaken.names, ["B_V1".to_string()].into_iter().collect());
    }

    /// A destructuring `let` types every name it binds.
    ///
    /// `let (checkpoint, digest) = read_checkpoint(..)?` is the exact shape
    /// that kept Trading's reserve and rollback routes ungated even once the
    /// two-arm match united: `checkpoint` was untyped, and `append_rollback`
    /// is carried by two types in the Dealer codec, so the resolver refused
    /// the bare name. Two types with one shared method name is what this test
    /// reproduces -- with a single `Book::read_page` the unique-name fallback
    /// would pass whether or not the tuple bound anything.
    #[test]
    fn a_destructuring_let_types_every_name_it_binds() {
        let scan = scan_body(
            "fn process() { let (book, digest) = load(); book.read_page(); } \
             fn load() -> Result<(Book, u8), E> { unimplemented!() } \
             impl Book { fn read_page(&self) { gate(A_V1); } } \
             impl Ledger { fn read_page(&self) { gate(B_V1); } }",
            &["A_V1", "B_V1"],
        );
        assert!(scan.calls.contains("Book::read_page"), "{:?}", scan.calls);
        assert!(!scan.calls.contains("Ledger::read_page"));
    }

    /// A tuple whose arity disagrees with the signature binds nothing.
    ///
    /// Binding the prefix would type one name with its neighbour's type,
    /// which is a guess wearing a resolution's clothes.
    #[test]
    fn a_tuple_let_of_the_wrong_width_binds_nothing() {
        let scan = scan_body(
            "fn process() { let (book, digest, extra) = load(); book.read_page(); } \
             fn load() -> Result<(Book, u8), E> { unimplemented!() } \
             impl Book { fn read_page(&self) { gate(A_V1); } } \
             impl Ledger { fn read_page(&self) { gate(B_V1); } }",
            &["A_V1", "B_V1"],
        );
        assert!(!scan.calls.contains("Book::read_page"), "{:?}", scan.calls);
        assert!(
            !scan.calls.contains("Ledger::read_page"),
            "{:?}",
            scan.calls
        );
    }

    /// A three-arm match stays variant-keyed, and unites nothing.
    ///
    /// The control for the case above: a pair holds two sides, and reading
    /// three arms as a pair would either drop one arm's set or publish a union
    /// of the two the code happened to write first. Resolution's
    /// `authenticate_core` is exactly this shape, and its founding-only arm
    /// must never reach the routes that admit a terminal or close a fund.
    #[test]
    fn a_three_arm_match_unites_nothing() {
        let scan = scan_body(
            "fn process(action: A) { match action { A::Create => gate(A_V1), A::Admit => gate(B_V1), A::Close => gate(C_V1) } }",
            &["A_V1", "B_V1", "C_V1"],
        );
        assert!(scan.unconditional.is_empty(), "{:?}", scan.unconditional);
        assert!(scan.alternatives.is_empty(), "{:?}", scan.alternatives);
    }

    /// An arm that gates nothing admits everything, so the pair is no gate.
    #[test]
    fn a_two_arm_match_with_one_bare_arm_is_not_a_gate() {
        let scan = scan_body(
            "fn process(action: A) { match action { A::Reserve => gate(A_V1), A::Rollback => 0 } }",
            &["A_V1"],
        );
        assert!(scan.alternatives.is_empty(), "{:?}", scan.alternatives);
        assert_eq!(
            scan.by_variant.get("Reserve"),
            Some(&["A_V1".to_string()].into_iter().collect())
        );
    }

    /// A call made only inside one arm is not a call every route makes.
    #[test]
    fn a_call_under_one_arm_is_keyed_to_that_arm() {
        let scan = scan_body(
            "fn process() { match action { A::Admit => admit(), A::Close => close() } }",
            &[],
        );
        assert!(scan.calls.is_empty(), "{:?}", scan.calls);
        assert_eq!(
            scan.calls_by_variant.get("Admit"),
            Some(&["admit".to_string()].into_iter().collect())
        );
        assert_eq!(
            scan.calls_by_variant.get("Close"),
            Some(&["close".to_string()].into_iter().collect())
        );
    }

    /// The no-state-machine declaration refuses to outlive its state model.
    ///
    /// Its positive control, and the reason it is a checked declaration
    /// rather than a footnote: a run in which nothing fires and a run in which
    /// the instrument is disconnected print the same thing. So both refutations
    /// are exercised here -- a program that also declares an admissible set,
    /// and a program whose sources read a known machine's discriminant.
    #[test]
    fn a_declaration_of_no_state_machine_is_refuted_by_its_own_sources() {
        let base =
            std::env::temp_dir().join(format!("dclutch-census-no-machine-{}", std::process::id()));
        let source = base.join("src");
        std::fs::create_dir_all(&source).expect("temp dir");
        std::fs::write(source.join("lib.rs"), "pub fn process() {}\n").expect("write");

        assert_eq!(
            no_persisted_discriminant("core", &source, &base, 0).expect("not declared"),
            None,
            "a program not on the list declares nothing"
        );
        assert!(
            no_persisted_discriminant("registry", &source, &base, 0)
                .expect("declared")
                .is_some()
        );
        let refuted = no_persisted_discriminant("registry", &source, &base, 3)
            .expect_err("an admission constant refutes the declaration");
        assert!(refuted.reason.contains("admissible-state set"));

        std::fs::write(
            source.join("lib.rs"),
            "pub fn process(phase: Phase) -> bool { phase == Phase::Open }\n",
        )
        .expect("write");
        let refuted = no_persisted_discriminant("registry", &source, &base, 0)
            .expect_err("a discriminant read refutes the declaration");
        assert_eq!(refuted.expression, "Phase");
        assert!(refuted.reason.contains("declaration is stale"));
        std::fs::remove_dir_all(&base).ok();
    }

    /// A method call is followed, and the receiver's type is what picks the
    /// body.
    ///
    /// `validate` collides two ways in Trading alone, so the name cannot pick
    /// it: `context.validate()` runs `Context::validate` because `context` is
    /// a parameter declared `Context`, and nothing else.
    #[test]
    fn a_method_call_is_resolved_through_the_receivers_type() {
        let scan = scan_body(
            "fn process(context: Context) { context.validate(); }
             struct Context {}
             impl Context { fn validate(&self) {} }
             struct Other {}
             impl Other { fn validate(&self) {} }",
            &[],
        );
        assert!(scan.calls.contains("Context::validate"), "{:?}", scan.calls);
        assert!(!scan.calls.contains("Other::validate"));
    }

    /// A receiver reached through a field carries that field's type.
    ///
    /// `input.context.validate(false)` is how the Direct venue writes it, and
    /// the type is two hops from the signature.
    #[test]
    fn a_field_receiver_is_typed_through_the_struct_it_belongs_to() {
        let scan = scan_body(
            "fn process(input: Input) { input.context.validate(false); }
             struct Input { context: Context }
             struct Context {}
             impl Context { fn validate(&self, terminal: bool) {} }",
            &[],
        );
        assert!(scan.calls.contains("Context::validate"), "{:?}", scan.calls);
    }

    /// A method name carried by two types resolves to neither.
    ///
    /// The same rule `CrateIndex::resolve` keeps for free functions: refusing
    /// is an under-count, and guessing attributes one venue's set to another
    /// venue's routes.
    #[test]
    fn an_ambiguous_method_name_on_an_untyped_receiver_is_refused() {
        let scan = scan_body(
            "fn process() { anything().validate(); }
             struct A {}
             impl A { fn validate(&self) {} }
             struct B {}
             impl B { fn validate(&self) {} }",
            &[],
        );
        assert!(
            !scan.calls.iter().any(|call| call.ends_with("::validate")),
            "{:?}",
            scan.calls
        );
    }

    /// A `for` body may run zero times, so it gates only what entered it.
    ///
    /// Measured, not theorised: Trading's commit route reads each locked
    /// reservation inside `for ordinal in 0..effect_count`, and a scenario
    /// that selected no Custody effect commits with the loop never entered.
    /// Read as unconditional, the route published a reservation set it does
    /// not always require.
    #[test]
    fn a_gate_inside_a_loop_is_not_every_routes_gate() {
        for source in [
            "fn process() { for ordinal in 0..count { check(A_V1); } authenticate(); }",
            "fn process() { while more { check(A_V1); } authenticate(); }",
        ] {
            let scan = scan_body(source, &["A_V1"]);
            assert!(scan.unconditional.is_empty(), "{:?}", scan.unconditional);
            assert!(scan.conditional.contains("A_V1"));
            assert!(!scan.calls.contains("check"), "{:?}", scan.calls);
            assert!(scan.calls.contains("authenticate"));
        }
    }

    /// An action route starts its scan at every ancestor as well as itself.
    ///
    /// Resolution writes the Market gate in `process_core_effect`, one match
    /// with four arms, and only then dispatches; the gate `process_create`
    /// passes is written in its parent and keyed by its own action. A scan
    /// that started at the child alone reported all four as ungated.
    #[test]
    fn a_routes_ancestry_is_itself_then_every_parent() {
        let route = |id: &str, handler: &str, parent: Option<&str>| Route {
            id: id.to_string(),
            kind: crate::model::RouteKind::Action,
            parent: parent.map(str::to_string),
            handler: handler.to_string(),
            selectors: Vec::new(),
            provenance: "x:1".into(),
            cfg: Vec::new(),
            admissible_prestates: Vec::new(),
        };
        let entry = route("r/entry", "entry", None);
        let child = route("r/child#Create", "create", Some("r/entry"));
        let index = AdmissionIndex::default();
        let crate_index = CrateIndex::default();
        let map = GuardMap::new(&index, &crate_index, &[entry, child.clone()]);
        assert_eq!(map.ancestry(&child), vec!["create", "entry"]);
    }

    /// A parent cycle stops rather than hanging the enumerator.
    #[test]
    fn a_parent_cycle_terminates() {
        let route = |id: &str, handler: &str, parent: &str| Route {
            id: id.to_string(),
            kind: crate::model::RouteKind::Action,
            parent: Some(parent.to_string()),
            handler: handler.to_string(),
            selectors: Vec::new(),
            provenance: "x:1".into(),
            cfg: Vec::new(),
            admissible_prestates: Vec::new(),
        };
        let left = route("r/left", "left", "r/right");
        let right = route("r/right", "right", "r/left");
        let index = AdmissionIndex::default();
        let crate_index = CrateIndex::default();
        let map = GuardMap::new(&index, &crate_index, &[left.clone(), right]);
        assert_eq!(map.ancestry(&left), vec!["left", "right"]);
    }

    /// A tag naming a tuple or a union of actions is not one variant, and a
    /// per-variant guard must not be attributed to it.
    #[test]
    fn only_a_single_variant_tag_keys_a_guard() {
        let route = |id: &str| Route {
            id: id.to_string(),
            kind: crate::model::RouteKind::Action,
            parent: None,
            handler: "m::process".into(),
            selectors: Vec::new(),
            provenance: "x:1".into(),
            cfg: Vec::new(),
            admissible_prestates: Vec::new(),
        };
        assert_eq!(
            route_variant(&route("core/m::process#AdmitTerminal")),
            Some("AdmitTerminal".to_string())
        );
        assert_eq!(route_variant(&route("core/m::process")), None);
        assert_eq!(route_variant(&route("core/m::process#(A::X,B::Y)")), None);
    }

    #[test]
    fn an_inline_handler_still_resolves_to_its_function() {
        let route = Route {
            id: "core/m::process#Tag".into(),
            kind: crate::model::RouteKind::Action,
            parent: None,
            handler: "m::process (inline: Tag)".into(),
            selectors: Vec::new(),
            provenance: "x:1".into(),
            cfg: Vec::new(),
            admissible_prestates: Vec::new(),
        };
        assert_eq!(handler_path(&route), "m::process");
    }
}
