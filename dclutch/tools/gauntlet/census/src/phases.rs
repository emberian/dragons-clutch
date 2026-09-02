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
];

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
    scan: GuardScan,
    /// The variants whose match arm we are inside. Empty is unconditional; an
    /// or-pattern arm holds every variant it names.
    variant: BTreeSet<String>,
    /// Whether we are inside a boolean branch, whose contents gate only the
    /// executions that took it.
    conditional: bool,
}

impl<'a> GuardVisitor<'a> {
    fn fresh(&self) -> GuardVisitor<'a> {
        GuardVisitor {
            index: self.index,
            scan: GuardScan::default(),
            variant: self.variant.clone(),
            conditional: self.conditional,
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
            let target = render_path(&path.path);
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
        visit::visit_expr_call(self, node);
    }

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

    fn scan(&mut self, module: &str, path: &str) -> Option<(String, &GuardScan)> {
        let function = self.crate_index.resolve_from(module, path)?;
        let key = format!("{}::{}", function.module, function.name);
        if !self.scans.contains_key(&key) {
            let mut visitor = GuardVisitor {
                index: self.index,
                scan: GuardScan::default(),
                variant: BTreeSet::new(),
                conditional: false,
            };
            visitor.visit_block(&function.block);
            self.scans.insert(key.clone(), visitor.scan);
        }
        let scan = self.scans.get(&key)?;
        Some((key, scan))
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
            let Some((key, scan)) = self.scan(&module, &path) else {
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
            let here = key
                .rsplit_once("::")
                .map_or(String::new(), |(module, _)| module.to_string());
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
            let Some((key, scan)) = self.scan(&module, &path) else {
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
            let here = key
                .rsplit_once("::")
                .map_or(String::new(), |(module, _)| module.to_string());
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
        let file = syn::parse_file(source).expect("parses");
        let Item::Fn(function) = &file.items[0] else {
            panic!("not a fn")
        };
        let mut visitor = GuardVisitor {
            index: &index,
            scan: GuardScan::default(),
            variant: BTreeSet::new(),
            conditional: false,
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
