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

/// The type whose constants declare a route's admissible Market prestates.
const ADMISSION_TYPE: &str = "MarketAdmissionV1";

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
    pub kind: AdmissionKind,
    pub phases: Vec<String>,
    pub prestates: Vec<Prestate>,
    pub provenance: Provenance,
}

/// Workspace-wide index of admissible-prestate constants, keyed by bare name.
///
/// Bare names are enough for the same reason the constant index uses them: the
/// declarations are globally unique by construction. A genuine collision is
/// refused rather than guessed at, so a route gated by a colliding name shows
/// as ungated -- visibly wrong -- instead of silently taking the wrong set.
#[derive(Default)]
pub struct AdmissionIndex {
    facts: BTreeMap<String, Vec<AdmissionFact>>,
    /// Constants of the right type written in a shape this module cannot read.
    pub unreadable: Vec<Unclassified>,
}

impl AdmissionIndex {
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
    if path.path.segments.last().map(|last| last.ident.to_string())
        != Some(ADMISSION_TYPE.to_string())
    {
        return;
    }
    let name = constant.ident.to_string();
    let provenance = at(relative, constant.ident.span());
    match read_initialiser(&constant.expr) {
        Ok(fact) => index.insert(
            name,
            AdmissionFact {
                kind: fact.0,
                phases: fact.1,
                prestates: fact.2,
                provenance,
            },
        ),
        Err(reason) => index.unreadable.push(Unclassified {
            context: format!("admissible prestates {name}"),
            expression: render_path(&constant.expr),
            provenance,
            reason,
        }),
    }
}

type ReadSet = (AdmissionKind, Vec<String>, Vec<Prestate>);

/// Read a `MarketAdmissionV1` initializer structurally.
fn read_initialiser(expr: &Expr) -> Result<ReadSet, String> {
    let Expr::Call(call) = strip(expr) else {
        return Err("initializer is not a MarketAdmissionV1 constructor call".into());
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
    match constructor.as_str() {
        "prestates" => {
            let mut prestates = Vec::new();
            let mut phases = Vec::new();
            for element in elements {
                let Expr::Tuple(tuple) = strip(element) else {
                    return Err("a prestate element is not a (Phase, Readiness) pair".into());
                };
                let mut parts = tuple.elems.iter();
                let (Some(phase), Some(readiness), None) =
                    (parts.next(), parts.next(), parts.next())
                else {
                    return Err("a prestate element is not a two-element tuple".into());
                };
                let phase = variant(phase, "Phase")?;
                let readiness = variant(readiness, "Readiness")?;
                if !phases.contains(&phase) {
                    phases.push(phase.clone());
                }
                prestates.push(Prestate { phase, readiness });
            }
            Ok((AdmissionKind::Prestates, phases, prestates))
        }
        "phases" => {
            let mut phases = Vec::new();
            for element in elements {
                let phase = variant(element, "Phase")?;
                if !phases.contains(&phase) {
                    phases.push(phase);
                }
            }
            Ok((AdmissionKind::Phases, phases, Vec::new()))
        }
        other => Err(format!(
            "unrecognised MarketAdmissionV1 constructor {other}"
        )),
    }
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
    /// Constants named outside any variant-keyed match arm: gates every route
    /// reaching this function passes.
    unconditional: BTreeSet<String>,
    /// `Enum::Variant => CONSTANT` arms, keyed by the variant's own name.
    by_variant: BTreeMap<String, BTreeSet<String>>,
    /// Call targets, for the descent.
    calls: BTreeSet<String>,
}

struct GuardVisitor<'a> {
    index: &'a AdmissionIndex,
    scan: GuardScan,
    /// The variant whose match arm we are inside, if any.
    variant: Option<String>,
}

impl<'a> GuardVisitor<'a> {
    fn note(&mut self, name: &str) {
        if self.index.resolve(name).is_none() {
            return;
        }
        match &self.variant {
            Some(variant) => {
                self.scan
                    .by_variant
                    .entry(variant.clone())
                    .or_default()
                    .insert(name.to_string());
            }
            None => {
                self.scan.unconditional.insert(name.to_string());
            }
        }
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
            self.scan.calls.insert(render_path(&path.path));
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_match(&mut self, node: &'ast syn::ExprMatch) {
        self.visit_expr(&node.expr);
        for arm in &node.arms {
            let outer = self.variant.take();
            self.variant = pattern_variant(&arm.pat).or(outer.clone());
            if let Some(guard) = &arm.guard {
                self.visit_expr(&guard.1);
            }
            self.visit_expr(&arm.body);
            self.variant = outer;
        }
    }
}

/// The variant name an arm pattern selects, when it selects exactly one.
///
/// An or-pattern selects several and is deliberately not read: a constant
/// under it gates every one of them, which is what the unconditional set
/// already means for the arm's own routes.
fn pattern_variant(pattern: &Pat) -> Option<String> {
    match pattern {
        Pat::Path(path) => path.path.segments.last().map(|s| s.ident.to_string()),
        Pat::TupleStruct(tuple) => tuple.path.segments.last().map(|s| s.ident.to_string()),
        Pat::Struct(structure) => structure.path.segments.last().map(|s| s.ident.to_string()),
        Pat::Ident(ident) => ident
            .subpat
            .as_ref()
            .and_then(|(_, inner)| pattern_variant(inner)),
        _ => None,
    }
}

/// Read the admissible-prestate gates each route in one program passes.
pub struct GuardMap<'a> {
    index: &'a AdmissionIndex,
    crate_index: &'a CrateIndex,
    /// Every route handler in the program: the descent stops at these, so one
    /// route's gates never leak into the entry route that dispatches to it.
    boundaries: BTreeSet<String>,
    /// Keyed by `module::function`, the identity a call is resolved from.
    scans: BTreeMap<String, GuardScan>,
}

impl<'a> GuardMap<'a> {
    pub fn new(index: &'a AdmissionIndex, crate_index: &'a CrateIndex, routes: &[Route]) -> Self {
        let boundaries = routes
            .iter()
            .map(|route| handler_path(route).to_string())
            .collect();
        Self {
            index,
            crate_index,
            boundaries,
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
                variant: None,
            };
            visitor.visit_block(&function.block);
            self.scans.insert(key.clone(), visitor.scan);
        }
        let scan = self.scans.get(&key)?;
        Some((key, scan))
    }

    /// The gates `route` passes, as declared constants.
    pub fn for_route(&mut self, route: &Route) -> Vec<PhaseAdmission> {
        let start = handler_path(route).to_string();
        let selected = route_variant(route);
        let mut names: BTreeSet<String> = BTreeSet::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut frontier = vec![(String::new(), start.clone(), 0usize)];
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
            let calls: Vec<String> = scan.calls.iter().cloned().collect();
            for call in calls {
                frontier.push((here.clone(), call, depth + 1));
            }
        }
        let mut admissions: Vec<PhaseAdmission> = names
            .into_iter()
            .filter_map(|name| {
                let fact = self.index.resolve(&name)?;
                Some(PhaseAdmission {
                    constant: name,
                    kind: fact.kind,
                    phases: fact.phases.clone(),
                    prestates: fact.prestates.clone(),
                    provenance: fact.provenance.clone(),
                })
            })
            .collect();
        admissions.sort_by(|left, right| left.constant.cmp(&right.constant));
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
        let file = syn::parse_file(source).expect("parses");
        let Item::Const(item) = &file.items[0] else {
            panic!("not a const")
        };
        read_initialiser(&item.expr)
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
