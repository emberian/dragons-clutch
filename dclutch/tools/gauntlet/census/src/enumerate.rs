//! Static enumeration of every program's public dispatch surface.
//!
//! The enumeration reads the Rust AST, not a hand-kept list, so a new dispatch
//! branch appears in the census the moment it is written and a deleted one
//! disappears. Anything in dispatch position that the classifier does not
//! recognise is emitted as `Unclassified` rather than dropped.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use proc_macro2::Span;
use quote_min::{render, render_path};
use syn::{Attribute, BinOp, Block, Expr, File, Item, ItemEnum, Pat, Stmt, spanned::Spanned};

use crate::{
    model::{
        Entrypoint, Inventory, NativeVariantSelector, ProgramSurface, Provenance, Refusal, Route,
        RouteKind, Selector, Unclassified,
    },
    phases::{AdmissionIndex, GuardMap},
    sources::Sources,
};

/// Minimal token rendering. `syn`'s `printing` feature gives us
/// `ToTokens`; we only ever need a one-line human-readable form.
pub(crate) mod quote_min {
    use quote::ToTokens;

    pub(crate) fn render<T: ToTokens>(value: &T) -> String {
        let text = value.to_token_stream().to_string();
        let mut collapsed = String::with_capacity(text.len());
        let mut last_space = false;
        for character in text.chars() {
            let space = character.is_whitespace();
            if space {
                if !last_space {
                    collapsed.push(' ');
                }
            } else {
                collapsed.push(character);
            }
            last_space = space;
        }
        collapsed.trim().to_string()
    }

    /// Rendered token streams put spaces around every punctuation token. Route
    /// identities must be stable and readable, and — more importantly — the
    /// selector classifier matches on path segments, so `a :: is_x` must
    /// normalise to `a::is_x` or a recogniser is silently missed.
    pub(crate) fn render_path<T: ToTokens>(value: &T) -> String {
        let joined = render(value)
            .replace(" :: ", "::")
            .replace(":: ", "::")
            .replace(" ::", "::");
        // `f::<10>` and `f::<11>` are one route with two width tags, not two
        // routes; the const-generic instantiation is not a wire discriminant.
        match joined.find("::<") {
            Some(index) => joined[..index].to_string(),
            None => joined,
        }
    }
}

const MAX_DISPATCH_DEPTH: usize = 2;

/// A constant the enumerator can resolve to a literal value.
#[derive(Clone, Debug)]
struct ConstantFact {
    value: ConstantValue,
    provenance: Provenance,
    /// The crate whose sources declare it, with `-` normalised to `_`.
    ///
    /// A bare name is enough to key most of this index, and deliberately so.
    /// It is not enough to FOLD one: `REQUEST_BYTES` is declared five times
    /// in four crates with four different values, and `retire_v1.rs` sums the
    /// Core codec's 72 while `open_selected_v3.rs` means its own. The crate is
    /// what a scoped lookup filters on when the bare name collides.
    krate: String,
}

#[derive(Clone, Debug)]
enum ConstantValue {
    Bytes { hex: String, ascii: Option<String> },
    Integer(i64),
}

/// Workspace-wide index of constants, keyed by bare identifier. Bare names are
/// enough here: the protocol's magics and width constants are globally unique
/// by construction, and a genuine collision is reported rather than guessed.
#[derive(Default)]
pub struct ConstantIndex {
    facts: BTreeMap<String, Vec<ConstantFact>>,
    /// Every crate directory the walk saw, `-` normalised to `_`.
    ///
    /// A path's first segment is a crate only if it names one. `resolution::
    /// RESOLUTION_CORE_INSTRUCTION_BYTES_V1` and `dclutch_market::
    /// REQUEST_BYTES` are both two segments, and only the second is qualified
    /// by a crate; without this set the first would be read as one.
    crates: BTreeSet<String>,
}

impl ConstantIndex {
    fn resolve(&self, name: &str) -> Option<&ConstantFact> {
        let facts = self.facts.get(name)?;
        // A colliding name is deliberately not guessed at.
        if facts.len() == 1 {
            facts.first()
        } else {
            None
        }
    }

    /// Resolve a path written inside `krate`, with that file's imports.
    ///
    /// The cautious bare-name rule first, because it needs no scope and is
    /// right whenever the tree declares a name once. Only a COLLIDING name
    /// consults the scope, and then it answers what the compiler answers: an
    /// explicit crate qualifier if the path carries one, else the crate the
    /// file imported the name from, else the file's own crate. If that crate
    /// does not declare the name exactly once, the lookup refuses.
    fn resolve_scoped(
        &self,
        path: &str,
        krate: &str,
        imports: &BTreeMap<String, String>,
    ) -> Option<&ConstantFact> {
        let name = path.rsplit("::").next().unwrap_or(path);
        let facts = self.facts.get(name)?;
        if facts.len() == 1 {
            return facts.first();
        }
        let segments: Vec<&str> = path.split("::").collect();
        let qualifier = segments
            .first()
            .filter(|_| segments.len() >= 2)
            .and_then(|first| self.crates.contains(*first).then_some((*first).to_string()))
            .or_else(|| {
                imports.get(name).and_then(|full| {
                    let first = full.split("::").next()?;
                    self.crates.contains(first).then(|| first.to_string())
                })
            })
            .unwrap_or_else(|| krate.to_string());
        let mut scoped = facts.iter().filter(|fact| fact.krate == qualifier);
        let first = scoped.next()?;
        if scoped.next().is_some() {
            return None;
        }
        Some(first)
    }
}

/// A `const` whose value is an expression over other constants.
///
/// Held back from the index until the names it sums are themselves known.
struct PendingConstant {
    name: String,
    expr: Expr,
    provenance: Provenance,
    krate: String,
    /// The declaring file's imports: leaf name -> the path it was imported by.
    imports: BTreeMap<String, String>,
}

/// Index every `const NAME: ... = <expr>;` in the tree that we can evaluate.
///
/// Two phases, because a width is rarely a literal. The first takes every
/// constant whose right-hand side IS one; the second folds the sums, to a
/// fixpoint, so that a constant may be written over names declared later or in
/// another crate. `RETIREMENT_CHECKPOINT_PREPARE_INSTRUCTION_BYTES_V1` is
/// `REQUEST_BYTES + RETIREMENT_BUNDLE_BYTES_V1 +
/// CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_REQUEST_BYTES_V1` across the Core and
/// Claims crates. Until cross-crate widths folded, routes the Core dispatch
/// separated by length were indistinguishable to every downstream reader.
pub fn index_constants(sources: &Sources) -> ConstantIndex {
    let mut index = ConstantIndex::default();
    let mut pending: Vec<PendingConstant> = Vec::new();
    for source in &sources.files {
        let krate = crate_of(&source.relative);
        index.crates.insert(krate.clone());
        let mut imports = BTreeMap::new();
        collect_imports(&source.file.items, &mut imports);
        index_constants_in_items(
            &source.file.items,
            &source.relative,
            &krate,
            &imports,
            &mut index,
            &mut pending,
        );
    }
    fold_pending_constants(&mut index, pending);
    index
}

/// `crates/dclutch-market/src/generated.rs` -> `dclutch_market`.
fn crate_of(relative: &str) -> String {
    relative
        .split('/')
        .nth(1)
        .unwrap_or_default()
        .replace('-', "_")
}

/// Flatten every `use` into leaf name -> full path.
///
/// Only the leaf matters: what a bare `REQUEST_BYTES` in this file MEANS is
/// the crate the file imported it from, which is the one thing a bare-name
/// index cannot know.
fn collect_imports(items: &[Item], out: &mut BTreeMap<String, String>) {
    for item in items {
        match item {
            Item::Use(use_item) => collect_use_tree(&use_item.tree, "", out),
            Item::Mod(module) => {
                if let Some((_, items)) = &module.content {
                    collect_imports(items, out);
                }
            }
            _ => {}
        }
    }
}

fn collect_use_tree(tree: &syn::UseTree, prefix: &str, out: &mut BTreeMap<String, String>) {
    let join = |segment: &str| {
        if prefix.is_empty() {
            segment.to_string()
        } else {
            format!("{prefix}::{segment}")
        }
    };
    match tree {
        syn::UseTree::Path(path) => {
            collect_use_tree(&path.tree, &join(&path.ident.to_string()), out);
        }
        syn::UseTree::Name(name) => {
            let leaf = name.ident.to_string();
            let full = join(&leaf);
            out.insert(leaf, full);
        }
        syn::UseTree::Rename(rename) => {
            let full = join(&rename.ident.to_string());
            out.insert(rename.rename.to_string(), full);
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                collect_use_tree(item, prefix, out);
            }
        }
        syn::UseTree::Glob(_) => {}
    }
}

/// Fold the expression-valued constants until nothing more resolves.
///
/// Bounded by construction: every pass either learns at least one constant or
/// is the last. A name that never folds simply stays out of the index, which
/// is the same "unresolved" a reader downstream already handles.
fn fold_pending_constants(index: &mut ConstantIndex, mut pending: Vec<PendingConstant>) {
    loop {
        let mut resolved = Vec::new();
        let mut still = Vec::new();
        for constant in pending {
            match evaluate_integer(&constant.expr, index, &constant.krate, &constant.imports) {
                Some(value) => resolved.push((constant, value)),
                None => still.push(constant),
            }
        }
        if resolved.is_empty() {
            return;
        }
        for (constant, value) in resolved {
            index
                .facts
                .entry(constant.name)
                .or_default()
                .push(ConstantFact {
                    value: ConstantValue::Integer(value),
                    provenance: constant.provenance,
                    krate: constant.krate,
                });
        }
        pending = still;
    }
}

/// Evaluate an integer constant expression over the index.
///
/// Deliberately small: the widths this exists for are sums, differences and
/// small products of named widths, and nothing here evaluates a call, an
/// `impl` associated item or a generic. An expression it does not model
/// returns `None`, and the constant stays unresolved rather than wrong.
fn evaluate_integer(
    expr: &Expr,
    index: &ConstantIndex,
    krate: &str,
    imports: &BTreeMap<String, String>,
) -> Option<i64> {
    match expr {
        Expr::Lit(literal) => match &literal.lit {
            syn::Lit::Int(int) => int.base10_parse::<i64>().ok(),
            _ => None,
        },
        Expr::Paren(paren) => evaluate_integer(&paren.expr, index, krate, imports),
        Expr::Group(group) => evaluate_integer(&group.expr, index, krate, imports),
        // `X as usize` over an integer changes no value this index holds.
        Expr::Cast(cast) => evaluate_integer(&cast.expr, index, krate, imports),
        Expr::Binary(binary) => {
            let left = evaluate_integer(&binary.left, index, krate, imports)?;
            let right = evaluate_integer(&binary.right, index, krate, imports)?;
            match binary.op {
                BinOp::Add(_) => left.checked_add(right),
                BinOp::Sub(_) => left.checked_sub(right),
                BinOp::Mul(_) => left.checked_mul(right),
                _ => None,
            }
        }
        Expr::Path(path) => {
            let text = render_path(&path.path);
            match &index.resolve_scoped(&text, krate, imports)?.value {
                ConstantValue::Integer(value) => Some(*value),
                ConstantValue::Bytes { .. } => None,
            }
        }
        _ => None,
    }
}

fn index_constants_in_items(
    items: &[Item],
    relative: &str,
    krate: &str,
    imports: &BTreeMap<String, String>,
    index: &mut ConstantIndex,
    pending: &mut Vec<PendingConstant>,
) {
    for item in items {
        match item {
            Item::Const(konst) => {
                record_constant(
                    &konst.ident,
                    &konst.expr,
                    relative,
                    krate,
                    imports,
                    index,
                    pending,
                );
            }
            Item::Mod(module) => {
                if let Some((_, items)) = &module.content {
                    index_constants_in_items(items, relative, krate, imports, index, pending);
                }
            }
            Item::Impl(block) => {
                for item in &block.items {
                    if let syn::ImplItem::Const(konst) = item {
                        record_constant(
                            &konst.ident,
                            &konst.expr,
                            relative,
                            krate,
                            imports,
                            index,
                            pending,
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

/// A literal goes straight into the index; anything else waits for the fold.
#[allow(clippy::too_many_arguments)]
fn record_constant(
    ident: &syn::Ident,
    expr: &Expr,
    relative: &str,
    krate: &str,
    imports: &BTreeMap<String, String>,
    index: &mut ConstantIndex,
    pending: &mut Vec<PendingConstant>,
) {
    let provenance = at(relative, ident.span());
    match constant_value(expr) {
        Some(value) => index
            .facts
            .entry(ident.to_string())
            .or_default()
            .push(ConstantFact {
                value,
                provenance,
                krate: krate.to_string(),
            }),
        None => pending.push(PendingConstant {
            name: ident.to_string(),
            expr: expr.clone(),
            provenance,
            krate: krate.to_string(),
            imports: imports.clone(),
        }),
    }
}

fn constant_value(expr: &Expr) -> Option<ConstantValue> {
    match expr {
        // `*b"DCLTCAT1"`
        Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
            constant_value(&unary.expr)
        }
        Expr::Lit(literal) => match &literal.lit {
            syn::Lit::ByteStr(bytes) => Some(bytes_value(&bytes.value())),
            syn::Lit::Int(int) => int.base10_parse::<i64>().ok().map(ConstantValue::Integer),
            _ => None,
        },
        // `[0x44, 0x43, ...]`
        Expr::Array(array) => {
            let mut bytes = Vec::with_capacity(array.elems.len());
            for element in &array.elems {
                match element {
                    Expr::Lit(literal) => match &literal.lit {
                        syn::Lit::Int(int) => bytes.push(int.base10_parse::<u8>().ok()?),
                        _ => return None,
                    },
                    _ => return None,
                }
            }
            Some(bytes_value(&bytes))
        }
        _ => None,
    }
}

fn bytes_value(bytes: &[u8]) -> ConstantValue {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    let ascii = bytes
        .iter()
        .all(u8::is_ascii_graphic)
        .then(|| String::from_utf8_lossy(bytes).into_owned());
    ConstantValue::Bytes { hex, ascii }
}

// ------------------------------------------------------------- crate walking

pub(crate) fn rust_sources(base: &Path) -> Result<Vec<PathBuf>, String> {
    let mut found = Vec::new();
    let mut stack = vec![base.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let entries = fs::read_dir(&directory)
            .map_err(|error| format!("read {}: {error}", directory.display()))?;
        for entry in entries {
            let entry = entry.map_err(|error| format!("read entry: {error}"))?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                // `target/` holds build output, never first-party source.
                if name != "target" && name != ".git" {
                    stack.push(path);
                }
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push(path);
            }
        }
    }
    found.sort();
    Ok(found)
}

pub(crate) fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

pub(crate) fn at(relative: &str, span: Span) -> Provenance {
    format!("{relative}:{}", span.start().line)
}

/// One function the enumerator can follow into, keyed by module path.
#[derive(Clone)]
pub(crate) struct FunctionFact {
    pub(crate) module: String,
    pub(crate) name: String,
    pub(crate) block: Block,
    relative: String,
    /// The type this is an inherent method of, for an `impl` item.
    ///
    /// A free function has none. This is what lets a call written
    /// `context.validate()` be resolved to the body that actually runs: the
    /// receiver's type is resolved first, and the method is looked up under
    /// it. Six Trading guards were invisible to the census for exactly the
    /// want of it -- `self.core_market.phase()` inside a `validate()` nothing
    /// followed.
    pub(crate) self_type: Option<String>,
    /// Each named parameter and the type it was declared with.
    ///
    /// The seed of receiver-type resolution: `context.validate(false)` names
    /// a parameter, and the signature is where its type is written.
    pub(crate) inputs: Vec<(String, String)>,
    /// Every named parameter, whatever its type.
    ///
    /// `inputs` is deliberately narrower -- it drops a parameter whose type
    /// this reader cannot name, because a receiver it cannot type is a
    /// receiver it must not resolve. But `&[u8]` is exactly such a type, and
    /// it is the type of EVERY instruction payload in this tree, so a rule
    /// phrased "hands its own parameter to one call" cannot be asked of
    /// `inputs` at all: the set is empty for every recogniser in the census.
    pub(crate) parameters: Vec<String>,
    /// The type this returns, unwrapped through one generic layer.
    ///
    /// `Result<PlanV2>` reads as `PlanV2`, because a `?` at the call site is
    /// what the caller binds. Used to type a `let` whose initializer is a
    /// call.
    pub(crate) output: Option<String>,
    /// The element types of a tuple return, empty when it returns no tuple.
    ///
    /// `let (checkpoint, digest) = read_checkpoint(..)?` binds two names, and
    /// `output` can carry one. Without this the first of them is untyped and
    /// its methods resolve only if the bare name happens to be unique in the
    /// whole first-party closure, which for `append_rollback` it is not.
    pub(crate) output_elements: Vec<Option<String>>,
    /// The function exists only when compiling for the SBF target.
    ///
    /// `#[cfg(target_os = "solana")]` is this tree's marker for loader
    /// plumbing: a function that vanishes on the host cannot be protocol
    /// dispatch, because every route is reachable from a host test. It is what
    /// lets [`unwrap_forwarding_shim`] tell a deserialization adapter apart
    /// from a program whose whole body is one route.
    machine_boundary: bool,
}

/// A parsed program crate: every function in it, indexed for call resolution.
///
/// It also carries each struct's field types, because a receiver is as often
/// a field of a parameter (`input.context.validate(..)`) as the parameter
/// itself, and a type that cannot be named cannot have its method followed.
#[derive(Default)]
pub(crate) struct CrateIndex {
    functions: Vec<FunctionFact>,
    /// One-hop public function re-exports, keyed by their callable module path.
    ///
    /// A dispatch can call `adapter::is_x` where `adapter.rs` says only
    /// `pub use codec::wire::is_x`. The compiler reaches the codec body, and
    /// the census must retain that canonical path so source-based consumers
    /// can reach it too. Several targets for one alias remain ambiguous.
    reexports: BTreeMap<String, Vec<String>>,
    /// Struct name -> field name -> the field's declared type.
    fields: BTreeMap<String, BTreeMap<String, String>>,
    /// `Enum::Variant` -> explicit source discriminants. A collision is
    /// retained so resolution can refuse instead of picking one.
    variants: BTreeMap<String, Vec<VariantFact>>,
}

#[derive(Clone)]
struct VariantFact {
    value: i64,
    provenance: Provenance,
}

impl CrateIndex {
    /// Resolve `a::b::name` (or bare `name`) to a function in this crate.
    /// A path whose module qualifier does not match anything is unresolved,
    /// which the caller reports rather than guessing at.
    pub(crate) fn resolve(&self, path: &str) -> Option<&FunctionFact> {
        if let Some(target) = self.reexport_target(path) {
            return self.resolve_declared(target);
        }
        self.resolve_declared(path)
    }

    /// Resolve a directly declared function without following an alias.
    fn resolve_declared(&self, path: &str) -> Option<&FunctionFact> {
        let segments: Vec<&str> = path.split("::").collect();
        let name = *segments.last()?;
        let qualifier = if segments.len() >= 2 {
            Some(segments[segments.len() - 2])
        } else {
            None
        };
        // Free functions and associated functions only. An INHERENT METHOD is
        // never what a path call names -- Rust resolves `open_hoard(..)` to
        // the free function and `state.open_hoard(..)` to the method, and they
        // routinely share a name because the adapter is named for the
        // transition it drives. Indexing methods without this made every such
        // pair ambiguous, and an ambiguous name is refused: Custody's five
        // projected route handlers stopped resolving at all, silently, while
        // the route count did not move. `resolve_from` reads a `Type::name`
        // qualifier through `resolve_method` before it ever gets here.
        let mut matches = self.functions.iter().filter(|fact| {
            fact.self_type.is_none()
                && fact.name == name
                && match qualifier {
                    None => true,
                    Some(qualifier) => {
                        qualifier == "self"
                            || qualifier == "crate"
                            || qualifier == "super"
                            || fact.module.rsplit("::").next() == Some(qualifier)
                    }
                }
        });
        let first = matches.next()?;
        if matches.next().is_some() {
            // Ambiguous: two same-named functions in same-named modules.
            return None;
        }
        Some(first)
    }

    /// The unique target of one public `use`, when that target exists here.
    ///
    /// Re-exports are followed exactly once. A chain or cycle is unresolved
    /// rather than turning source enumeration into package resolution.
    fn reexport_target(&self, path: &str) -> Option<&str> {
        let targets = self.reexports.get(path)?;
        let [target] = targets.as_slice() else {
            return None;
        };
        self.resolve_declared(target).is_some().then_some(target)
    }

    /// Preserve the function identity Rust publicly exposes for a predicate.
    fn canonical_function_path<'a>(&'a self, path: &'a str) -> &'a str {
        self.reexport_target(path).unwrap_or(path)
    }

    /// Resolve a call written inside `module`, preferring that module's own
    /// item.
    ///
    /// A bare `authenticate_market(..)` in `retire_v1` names `retire_v1`'s
    /// function, not `fixed_role`'s and not the handoff module's, and Rust
    /// resolves it that way. [`CrateIndex::resolve`] cannot: three functions
    /// share the name, so it refuses to guess -- correctly, for a dispatch
    /// forward it is asked to follow from nowhere in particular. A guard scan
    /// walking a known function's body does know where it stands, so it can
    /// answer what the compiler answers. Falls back to the cautious rule when
    /// the caller's own module has no such item.
    pub(crate) fn resolve_from(&self, module: &str, path: &str) -> Option<&FunctionFact> {
        // `Type::method` is how a resolved method call is spelled, and it is
        // also how an associated function is written in source. Either way the
        // qualifier names a type, so try that reading before the module one.
        if let Some((qualifier, name)) = path.rsplit_once("::")
            && !qualifier.contains("::")
            && let Some(method) = self.resolve_method(qualifier, name)
        {
            return Some(method);
        }
        if !path.contains("::") {
            let mut local = self
                .functions
                .iter()
                .filter(|fact| fact.name == path && fact.module == module);
            if let Some(first) = local.next()
                && local.next().is_none()
            {
                return Some(first);
            }
        }
        self.resolve(path)
    }

    /// The inherent method `name` on `self_type`, if exactly one is written.
    ///
    /// Exact by construction: an inherent method is unique on its type, and
    /// two `impl` blocks writing the same name on the same type do not
    /// compile. Two DIFFERENT types may each have a `validate`, which is
    /// precisely why the receiver's type has to be resolved before the lookup
    /// -- `CrateIndex::resolve` refuses `validate` outright, and is right to.
    pub(crate) fn resolve_method(&self, self_type: &str, name: &str) -> Option<&FunctionFact> {
        let mut matches = self
            .functions
            .iter()
            .filter(|fact| fact.name == name && fact.self_type.as_deref() == Some(self_type));
        let first = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        Some(first)
    }

    /// The sole inherent method named `name` anywhere in this crate.
    ///
    /// The fallback for a receiver whose type this index cannot name -- a
    /// value from a dependency crate, or an expression shape not modelled.
    /// It REFUSES on ambiguity rather than picking one, which is the same
    /// rule [`CrateIndex::resolve`] keeps for free functions: `validate`
    /// collides two ways in Trading alone, so a guesser would have attributed
    /// one venue's phase set to another's routes.
    pub(crate) fn sole_method(&self, name: &str) -> Option<&FunctionFact> {
        let mut matches = self
            .functions
            .iter()
            .filter(|fact| fact.name == name && fact.self_type.is_some());
        let first = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        Some(first)
    }

    /// The declared type of one field of one struct.
    pub(crate) fn field_type(&self, owner: &str, field: &str) -> Option<&str> {
        self.fields.get(owner)?.get(field).map(String::as_str)
    }

    fn variant(&self, path: &str) -> Option<&VariantFact> {
        let mut segments = path.rsplit("::");
        let variant = segments.next()?;
        let enumeration = segments.next()?;
        let facts = self.variants.get(&format!("{enumeration}::{variant}"))?;
        (facts.len() == 1).then(|| &facts[0])
    }
}

fn module_path_for(crate_src: &Path, file: &Path) -> String {
    let relative = file.strip_prefix(crate_src).unwrap_or(file);
    let mut segments: Vec<String> = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect();
    let Some(last) = segments.pop() else {
        return String::new();
    };
    let stem = last.trim_end_matches(".rs");
    if stem != "mod" && stem != "lib" && stem != "main" {
        segments.push(stem.to_string());
    }
    segments.join("::")
}

fn index_crate(sources: &Sources, crate_src: &Path) -> CrateIndex {
    index_sources(sources, &[crate_src.to_path_buf()])
}

/// Index every `src` root given, in order, as one namespace.
fn index_sources(sources: &Sources, roots: &[PathBuf]) -> CrateIndex {
    let mut index = CrateIndex::default();
    for crate_src in roots {
        for source in sources.under(crate_src) {
            let module = module_path_for(crate_src, &source.path);
            collect_functions(&source.file.items, &module, &source.relative, &mut index);
        }
    }
    index
}

/// The type a declaration names, unwrapped through one generic layer.
///
/// `Result<PlanV2>` is `PlanV2` and `&DirectContextV2` is `DirectContextV2`,
/// because what a caller binds after a `?` or a borrow is the inner type. A
/// shape with no single named type -- a tuple, a closure, `impl Trait` --
/// yields nothing, and the receiver is then resolved by the fallback rule or
/// not at all.
pub(crate) fn declared_type_name(declared: &syn::Type) -> Option<String> {
    match declared {
        syn::Type::Path(path) => {
            let last = path.path.segments.last()?;
            if let syn::PathArguments::AngleBracketed(arguments) = &last.arguments
                && let Some(syn::GenericArgument::Type(inner)) = arguments
                    .args
                    .iter()
                    .find(|argument| matches!(argument, syn::GenericArgument::Type(_)))
            {
                return declared_type_name(inner);
            }
            Some(last.ident.to_string())
        }
        syn::Type::Reference(reference) => declared_type_name(&reference.elem),
        syn::Type::Paren(inner) => declared_type_name(&inner.elem),
        syn::Type::Group(inner) => declared_type_name(&inner.elem),
        _ => None,
    }
}

/// One function's named parameters and their declared types.
pub(crate) fn signature_inputs(signature: &syn::Signature) -> Vec<(String, String)> {
    let mut inputs = Vec::new();
    for argument in &signature.inputs {
        let syn::FnArg::Typed(typed) = argument else {
            continue;
        };
        let syn::Pat::Ident(ident) = typed.pat.as_ref() else {
            continue;
        };
        if let Some(declared) = declared_type_name(&typed.ty) {
            inputs.push((ident.ident.to_string(), declared));
        }
    }
    inputs
}

/// Every named parameter of a signature, whatever its declared type.
pub(crate) fn signature_parameters(signature: &syn::Signature) -> Vec<String> {
    signature
        .inputs
        .iter()
        .filter_map(|argument| {
            let syn::FnArg::Typed(typed) = argument else {
                return None;
            };
            let syn::Pat::Ident(ident) = typed.pat.as_ref() else {
                return None;
            };
            Some(ident.ident.to_string())
        })
        .collect()
}

fn signature_output(signature: &syn::Signature, self_type: Option<&str>) -> Option<String> {
    let syn::ReturnType::Type(_, declared) = &signature.output else {
        return None;
    };
    let named = declared_type_name(declared)?;
    if named == "Self" {
        return self_type.map(str::to_string);
    }
    Some(named)
}

/// The element types of a tuple return, one entry per element.
///
/// `declared_type_name` unwraps `Result<T, E>` to `T` and gives up on a tuple,
/// because a tuple is not one name. But a `let (a, b) = f()?;` binds as many
/// names as `f` returns elements, and until this existed every one of them was
/// untyped -- which is how Trading's reservation body lost its gate. Its
/// `checkpoint` comes from `let (checkpoint, digest) = read_checkpoint(..)?`,
/// so `checkpoint.append_rollback(..)` fell through to the unique-name rule,
/// and `append_rollback` is carried by two types in the Dealer codec. The
/// resolver refused it, correctly, for the want of a type that was written in
/// the signature all along.
///
/// `None` for anything that is not a tuple after one generic layer, so a
/// non-tuple return is unchanged and a tuple element this file cannot name
/// stays `None` in place rather than shifting its siblings.
fn signature_output_elements(
    signature: &syn::Signature,
    self_type: Option<&str>,
) -> Vec<Option<String>> {
    let syn::ReturnType::Type(_, declared) = &signature.output else {
        return Vec::new();
    };
    let Some(tuple) = unwrap_to_tuple(declared) else {
        return Vec::new();
    };
    tuple
        .elems
        .iter()
        .map(|element| match declared_type_name(element) {
            Some(named) if named == "Self" => self_type.map(str::to_string),
            other => other,
        })
        .collect()
}

/// One declared type as a tuple, looking through one generic layer.
///
/// `(A, B)` is one; so is `Result<(A, B), E>`, which is what every fallible
/// helper in a program crate is written as.
fn unwrap_to_tuple(declared: &syn::Type) -> Option<&syn::TypeTuple> {
    match declared {
        syn::Type::Tuple(tuple) => Some(tuple),
        syn::Type::Paren(inner) => unwrap_to_tuple(&inner.elem),
        syn::Type::Group(inner) => unwrap_to_tuple(&inner.elem),
        syn::Type::Reference(reference) => unwrap_to_tuple(&reference.elem),
        syn::Type::Path(path) => {
            let last = path.path.segments.last()?;
            let syn::PathArguments::AngleBracketed(arguments) = &last.arguments else {
                return None;
            };
            arguments
                .args
                .iter()
                .find_map(|argument| match argument {
                    syn::GenericArgument::Type(inner) => Some(inner),
                    _ => None,
                })
                .and_then(unwrap_to_tuple)
        }
        _ => None,
    }
}

fn function_fact(
    function: &syn::ItemFn,
    module: &str,
    relative: &str,
    self_type: Option<&str>,
) -> FunctionFact {
    FunctionFact {
        module: module.to_string(),
        name: function.sig.ident.to_string(),
        block: (*function.block).clone(),
        relative: relative.to_string(),
        self_type: self_type.map(str::to_string),
        inputs: signature_inputs(&function.sig),
        parameters: signature_parameters(&function.sig),
        output: signature_output(&function.sig, self_type),
        output_elements: signature_output_elements(&function.sig, self_type),
        machine_boundary: cfg_texts(&function.attrs)
            .iter()
            .any(|text| text.contains("target_os = \"solana\"")),
    }
}

/// Every first-party runtime dependency a program's manifest reaches,
/// transitively.
///
/// A guard is written where its state machine's discriminant lives, which for
/// six of this tree's machines is a codec crate rather than a program. The
/// dispatch index is deliberately the program's own crate -- adding foreign
/// functions to it makes an entrypoint name ambiguous and the route walk
/// refuses, which is right -- so the guard descent gets its own wider index
/// instead, and this is what tells it how wide.
///
/// Read from dependency sections in the manifests rather than globbed, so a
/// crate the program does not link can never contribute a gate to its routes.
/// In particular, dev dependencies compile native tests rather than the
/// program. Treating their path entries as runtime edges can even walk back
/// into the program through an optional operator dependency, indexing every
/// program guard twice and making each one unresolvable.
fn first_party_dependencies(root: &Path, manifest: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    let mut frontier = vec![manifest.to_path_buf()];
    while let Some(manifest) = frontier.pop() {
        let Ok(text) = fs::read_to_string(&manifest) else {
            continue;
        };
        let Some(directory) = manifest.parent() else {
            continue;
        };
        let mut dependency_section = false;
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                let header = trimmed
                    .split_once('#')
                    .map_or(trimmed, |(before, _)| before.trim_end());
                dependency_section = header == "[dependencies]"
                    || header.starts_with("[dependencies.")
                    || (header.starts_with("[target.")
                        && (header.ends_with(".dependencies]")
                            || header.contains(".dependencies.")));
                continue;
            }
            if !dependency_section || trimmed.starts_with('#') {
                continue;
            }
            let Some(rest) = line.split_once("path = \"") else {
                continue;
            };
            let Some((relative, _)) = rest.1.split_once('"') else {
                continue;
            };
            // `path = "src/lib.rs"` in a `[lib]` section is not a dependency.
            if !relative.starts_with("..") {
                continue;
            }
            let Ok(crate_root) = directory.join(relative).canonicalize() else {
                continue;
            };
            if !crate_root.starts_with(root) || !seen.insert(crate_root.clone()) {
                continue;
            }
            let source = crate_root.join("src");
            if source.is_dir() {
                found.push(source);
            }
            frontier.push(crate_root.join("Cargo.toml"));
        }
    }
    found.sort();
    found
}

#[cfg(test)]
mod dependency_source_tests {
    use super::first_party_dependencies;
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "dclutch-census-dependency-sources-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create fixture root");
            Self(path)
        }

        fn crate_dir(&self, relative: &str, manifest: &str) -> PathBuf {
            let directory = self.0.join(relative);
            fs::create_dir_all(directory.join("src")).expect("create fixture crate");
            fs::write(directory.join("Cargo.toml"), manifest).expect("write fixture manifest");
            directory
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove fixture");
        }
    }

    /// A native test helper is not part of the program's chain dispatch
    /// namespace. In particular, an optional edge from that helper back to
    /// the program must not index every program function a second time and
    /// make its guards ambiguous.
    #[test]
    fn dev_dependency_is_not_a_program_guard_dependency() {
        let fixture = Fixture::new();
        let trading = fixture.crate_dir(
            "programs/trading",
            "[dependencies] # ordinary runtime table\n\
             codec = { path = \"../../crates/codec\" }\n\
             [target.'cfg(target_os = \"solana\")'.dependencies] # target runtime table\n\
             target-codec = { path = \"../../crates/target-codec\" }\n\
             [dev-dependencies] # native tests only\n\
             operator = { path = \"../../crates/operator\" }\n\
             [build-dependencies] # build process only\n\
             builder = { path = \"../../crates/builder\" }\n",
        );
        let codec = fixture.crate_dir("crates/codec", "[dependencies]\n");
        let target_codec = fixture.crate_dir("crates/target-codec", "[dependencies]\n");
        fixture.crate_dir(
            "crates/operator",
            "[dependencies]\ntrading = { path = \"../../programs/trading\", optional = true }\n",
        );
        fixture.crate_dir(
            "crates/builder",
            "[dependencies]\ntrading = { path = \"../../programs/trading\", optional = true }\n",
        );
        let root = fixture
            .path()
            .canonicalize()
            .expect("canonical fixture root");

        assert_eq!(
            first_party_dependencies(&root, &trading.join("Cargo.toml")),
            vec![
                codec
                    .join("src")
                    .canonicalize()
                    .expect("canonical codec src"),
                target_codec
                    .join("src")
                    .canonicalize()
                    .expect("canonical target codec src"),
            ]
        );
    }
}

/// Index one source text as if it were a whole crate.
///
/// The guard-scan tests need a real index rather than an empty one now that a
/// method call is resolved through it.
#[cfg(test)]
pub(crate) fn index_source(module: &str, source: &str) -> CrateIndex {
    let file = syn::parse_file(source).expect("parses");
    let mut index = CrateIndex::default();
    collect_functions(&file.items, module, "src/lib.rs", &mut index);
    index
}

fn collect_functions(items: &[Item], module: &str, relative: &str, out: &mut CrateIndex) {
    for item in items {
        match item {
            Item::Fn(function) => out
                .functions
                .push(function_fact(function, module, relative, None)),
            // An inherent `impl`'s methods are indexed under the type they are
            // written on. A trait `impl` is deliberately skipped: the same
            // method name then belongs to many types, and a census that
            // followed one would be guessing which.
            Item::Impl(block) => {
                if block.trait_.is_some() {
                    continue;
                }
                let Some(owner) = declared_type_name(&block.self_ty) else {
                    continue;
                };
                for inner in &block.items {
                    let syn::ImplItem::Fn(method) = inner else {
                        continue;
                    };
                    let self_type = Some(owner.as_str());
                    out.functions.push(FunctionFact {
                        module: module.to_string(),
                        name: method.sig.ident.to_string(),
                        block: method.block.clone(),
                        relative: relative.to_string(),
                        self_type: self_type.map(str::to_string),
                        inputs: signature_inputs(&method.sig),
                        parameters: signature_parameters(&method.sig),
                        output: signature_output(&method.sig, self_type),
                        output_elements: signature_output_elements(&method.sig, self_type),
                        machine_boundary: cfg_texts(&method.attrs)
                            .iter()
                            .any(|text| text.contains("target_os = \"solana\"")),
                    });
                }
            }
            Item::Struct(structure) => {
                let syn::Fields::Named(named) = &structure.fields else {
                    continue;
                };
                let owner = structure.ident.to_string();
                let entry = out.fields.entry(owner).or_default();
                for field in &named.named {
                    let (Some(name), Some(declared)) =
                        (field.ident.as_ref(), declared_type_name(&field.ty))
                    else {
                        continue;
                    };
                    entry.insert(name.to_string(), declared);
                }
            }
            Item::Enum(enumeration) => {
                let owner = enumeration.ident.to_string();
                for variant in &enumeration.variants {
                    let Some((_, expression)) = &variant.discriminant else {
                        continue;
                    };
                    let Some(ConstantValue::Integer(value)) = constant_value(expression) else {
                        continue;
                    };
                    out.variants
                        .entry(format!("{owner}::{}", variant.ident))
                        .or_default()
                        .push(VariantFact {
                            value,
                            provenance: at(relative, variant.ident.span()),
                        });
                }
            }
            Item::Use(import) if matches!(import.vis, syn::Visibility::Public(_)) => {
                collect_function_reexports(&import.tree, "", module, out);
            }
            Item::Mod(inner) => {
                // Skip `#[cfg(test)]` modules: test code is not a public entry.
                if has_cfg_test(&inner.attrs) {
                    continue;
                }
                if let Some((_, items)) = &inner.content {
                    let nested = if module.is_empty() {
                        inner.ident.to_string()
                    } else {
                        format!("{module}::{}", inner.ident)
                    };
                    collect_functions(items, &nested, relative, out);
                }
            }
            _ => {}
        }
    }
}

/// Flatten named public imports into callable alias -> declared target.
fn collect_function_reexports(
    tree: &syn::UseTree,
    prefix: &str,
    module: &str,
    out: &mut CrateIndex,
) {
    let join = |segment: &str| {
        if prefix.is_empty() {
            segment.to_string()
        } else {
            format!("{prefix}::{segment}")
        }
    };
    let alias_path = |alias: &str| {
        if module.is_empty() {
            alias.to_string()
        } else {
            format!("{module}::{alias}")
        }
    };
    match tree {
        syn::UseTree::Path(path) => {
            collect_function_reexports(&path.tree, &join(&path.ident.to_string()), module, out);
        }
        syn::UseTree::Name(name) => {
            let leaf = name.ident.to_string();
            out.reexports
                .entry(alias_path(&leaf))
                .or_default()
                .push(join(&leaf));
        }
        syn::UseTree::Rename(rename) => {
            out.reexports
                .entry(alias_path(&rename.rename.to_string()))
                .or_default()
                .push(join(&rename.ident.to_string()));
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                collect_function_reexports(item, prefix, module, out);
            }
        }
        syn::UseTree::Glob(_) => {}
    }
}

fn has_cfg_test(attributes: &[Attribute]) -> bool {
    attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("cfg"))
        .any(|attribute| render(attribute).contains("test"))
}

fn cfg_texts(attributes: &[Attribute]) -> Vec<String> {
    attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("cfg"))
        .map(render)
        .collect()
}

// --------------------------------------------------------------- entrypoints

fn find_entrypoints(items: &[Item], relative: &str) -> Vec<(String, String, Provenance)> {
    let mut found = Vec::new();
    for item in items {
        match item {
            Item::Macro(macro_item) => {
                let Some(name) = macro_item.mac.path.segments.last() else {
                    continue;
                };
                let name = name.ident.to_string();
                if name == "entrypoint" || name == "entrypoint_no_alloc" {
                    let target = render(&macro_item.mac.tokens);
                    found.push((name, target, at(relative, macro_item.mac.span())));
                }
            }
            // A raw SBF entrypoint declared without the macro, e.g.
            // `#[no_mangle] pub unsafe extern "C" fn entrypoint(input: *mut u8)`.
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                let is_extern_c = function.sig.abi.is_some();
                if name == "entrypoint" && is_extern_c {
                    found.push((
                        "extern-C".to_string(),
                        name,
                        at(relative, function.sig.ident.span()),
                    ));
                }
            }
            Item::Mod(inner) => {
                if let Some((_, items)) = &inner.content
                    && !has_cfg_test(&inner.attrs)
                {
                    found.extend(find_entrypoints(items, relative));
                }
            }
            _ => {}
        }
    }
    found
}

// ------------------------------------------------------------------ refusals

pub(crate) fn collect_refusals(
    items: &[Item],
    label: &str,
    relative: &str,
    out: &mut Vec<Refusal>,
) {
    for item in items {
        match item {
            Item::Enum(enumeration) => {
                if !enumeration.ident.to_string().contains("Error") {
                    continue;
                }
                if !enumeration
                    .attrs
                    .iter()
                    .any(|attribute| attribute.path().is_ident("repr"))
                {
                    continue;
                }
                push_refusals(enumeration, label, relative, out);
            }
            Item::Mod(inner) => {
                if let Some((_, items)) = &inner.content
                    && !has_cfg_test(&inner.attrs)
                {
                    collect_refusals(items, label, relative, out);
                }
            }
            _ => {}
        }
    }
}

fn push_refusals(enumeration: &ItemEnum, label: &str, relative: &str, out: &mut Vec<Refusal>) {
    let enum_name = enumeration.ident.to_string();
    let mut implicit: i64 = 0;
    for variant in &enumeration.variants {
        let code = match &variant.discriminant {
            Some((_, expr)) => match constant_value(expr) {
                Some(ConstantValue::Integer(value)) => Some(value),
                _ => None,
            },
            None => Some(implicit),
        };
        if let Some(code) = code {
            implicit = code.saturating_add(1);
        }
        let variant_name = variant.ident.to_string();
        let (summary, detail) = doc_text(&variant.attrs);
        out.push(Refusal {
            id: format!("{label}/{enum_name}::{variant_name}"),
            enum_name: enum_name.clone(),
            variant: variant_name,
            code,
            summary,
            detail,
            provenance: at(relative, variant.ident.span()),
        });
    }
}

/// Split a variant's doc comment the way rustdoc does: the first paragraph is
/// the summary, everything after the first blank line is rationale.
///
/// Joining the two produces a caption that opens with the meaning and then
/// lectures, which is wrong in every place a caption is rendered. Keeping them
/// apart costs one field and loses nothing.
fn doc_text(attributes: &[Attribute]) -> (Option<String>, Option<String>) {
    let mut lines = Vec::new();
    for attribute in attributes {
        if !attribute.path().is_ident("doc") {
            continue;
        }
        if let syn::Meta::NameValue(pair) = &attribute.meta
            && let Expr::Lit(literal) = &pair.value
            && let syn::Lit::Str(text) = &literal.lit
        {
            lines.push(text.value().trim().to_string());
        }
    }
    let mut paragraphs = lines
        .split(String::is_empty)
        .map(|block| block.join(" ").trim().to_string())
        .filter(|block| !block.is_empty());
    let summary = paragraphs.next();
    let rest = paragraphs.collect::<Vec<_>>();
    let detail = if rest.is_empty() {
        None
    } else {
        Some(rest.join("\n\n"))
    };
    (summary, detail)
}

// ------------------------------------------------------------------ dispatch

struct DispatchWalk<'a> {
    label: &'a str,
    index: &'a CrateIndex,
    /// This program's crate PLUS every first-party crate it depends on.
    ///
    /// The dispatch walk itself deliberately reads only the program's own
    /// crate (an ambiguous entrypoint name is refused rather than guessed at),
    /// and that is unchanged. This wider namespace is used for exactly one
    /// thing: resolving an `is_*(data)` guard to the body that states which
    /// bytes it matches on.
    predicates: &'a CrateIndex,
    constants: &'a ConstantIndex,
    routes: Vec<Route>,
    unclassified: Vec<Unclassified>,
    visited: BTreeSet<String>,
    /// How many ENTRY-level dispatch forwards have been declined for want of a
    /// discriminant. The census's own blind-spot counter: a guarded region that
    /// hands the wire to a named handler, under a condition this walk read no
    /// wire bytes out of, is a route the register does not contain and does not
    /// say it is missing. Only `depth == 0` increments it, so a handler's own
    /// internal calls -- followed at `depth + 1` -- cannot be mistaken for one.
    declined_entries: usize,
}

impl DispatchWalk<'_> {
    fn walk_function(
        &mut self,
        function: &FunctionFact,
        depth: usize,
        parent: Option<&str>,
        inherited: &[Selector],
        cfg: &[String],
    ) {
        let key = format!("{}::{}", function.module, function.name);
        if depth > MAX_DISPATCH_DEPTH || !self.visited.insert(key) {
            return;
        }
        let context = qualified(&function.module, &function.name);
        self.walk_block(
            &function.block,
            &function.relative,
            &context,
            depth,
            parent,
            inherited,
            cfg,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_block(
        &mut self,
        block: &Block,
        relative: &str,
        context: &str,
        depth: usize,
        parent: Option<&str>,
        inherited: &[Selector],
        cfg: &[String],
    ) {
        let count = block.stmts.len();
        for (position, statement) in block.stmts.iter().enumerate() {
            let tail = position + 1 == count;
            match statement {
                Stmt::Expr(expr, semi) => {
                    // A dispatch-position expression is a `return`, a tail
                    // expression, or an `if`/`match` at statement position.
                    let dispatch_position = tail || semi.is_none() || is_control(expr);
                    if dispatch_position {
                        self.walk_expr(expr, relative, context, depth, parent, inherited, cfg);
                    }
                }
                // A `let` initialiser is walked when it is a DISPATCH, and
                // skipped when it is a lookup table. See
                // `local_initialiser_dispatches` for why the distinction is the
                // whole content of this arm.
                Stmt::Local(local) => {
                    if let Some(init) = &local.init
                        && local_initialiser_dispatches(&init.expr)
                    {
                        self.walk_expr(
                            &init.expr, relative, context, depth, parent, inherited, cfg,
                        );
                    }
                }
                Stmt::Item(_) | Stmt::Macro(_) => {}
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_expr(
        &mut self,
        expr: &Expr,
        relative: &str,
        context: &str,
        depth: usize,
        parent: Option<&str>,
        inherited: &[Selector],
        cfg: &[String],
    ) {
        match expr {
            Expr::If(branch) => {
                let mut branch_cfg = cfg.to_vec();
                branch_cfg.extend(cfg_texts(&branch.attrs));
                let mut selectors = inherited.to_vec();
                selectors.extend(self.selectors_from(&branch.cond));
                let before = self.routes.len();
                let declined_before = self.declined_entries;
                self.walk_block(
                    &branch.then_branch,
                    relative,
                    context,
                    depth,
                    parent,
                    &selectors,
                    &branch_cfg,
                );
                // THE BLIND SPOT, said out loud. A guard at the entry that
                // forwards the wire to a named handler, and out of which this
                // walk read no discriminant, is a live route the register does
                // not contain. Until 2026-09-07 it was dropped in silence --
                // `push_route` declined it and the denominator simply did not
                // move -- so `custody/upkeep_vault_v1::process` and
                // `custody/protocol_parameters_v1::process` were absent from a
                // register that looked complete, and the only way anyone found
                // out was planting a blocking entry and watching the census
                // call it stale. A census that cannot enumerate its own blind
                // spot is the defect; this is the enumeration.
                if depth == 0 && self.declined_entries > declined_before {
                    self.report(
                        context,
                        &branch.cond,
                        relative,
                        "entry dispatch guard forwards to a handler and states no wire \
                         discriminant this census can read; the route behind it is NOT \
                         enumerated. Teach `scan_condition` this shape",
                    );
                }
                // A guarded region that names no handler is still a route: the
                // guard is a wire discriminant and the body is the route body.
                // Dropping it would make an inline-handled instruction shape
                // invisible, which is exactly the silence the census exists to
                // remove.
                if self.routes.len() == before && depth == 0 && selects_a_route(&selectors, depth) {
                    self.push_inline_route(
                        context,
                        depth,
                        parent,
                        &selectors,
                        relative,
                        branch.cond.span(),
                        &branch_cfg,
                    );
                }
                if let Some((_, otherwise)) = &branch.else_branch {
                    let mut fallthrough = inherited.to_vec();
                    fallthrough.push(Selector::Fallthrough);
                    self.walk_expr(
                        otherwise,
                        relative,
                        context,
                        depth,
                        parent,
                        &fallthrough,
                        &branch_cfg,
                    );
                }
            }
            Expr::Block(block) => self.walk_block(
                &block.block,
                relative,
                context,
                depth,
                parent,
                inherited,
                cfg,
            ),
            Expr::Return(ret) => {
                if let Some(inner) = &ret.expr {
                    self.record_dispatch(inner, relative, context, depth, parent, inherited, cfg);
                }
            }
            Expr::Match(matched) => {
                for arm in &matched.arms {
                    let mut selectors = inherited.to_vec();
                    selectors.extend(self.arm_selectors(&arm.pat));
                    // `Some(magic) if magic == X` puts the real discriminant in
                    // the guard, not the pattern.
                    if let Some((_, guard)) = &arm.guard {
                        selectors.extend(self.selectors_from(guard));
                    }
                    let tagged = selectors.iter().any(is_wire_discriminant);
                    let mut arm_cfg = cfg.to_vec();
                    arm_cfg.extend(cfg_texts(&arm.attrs));
                    let before = self.routes.len();
                    self.record_dispatch(
                        &arm.body, relative, context, depth, parent, &selectors, &arm_cfg,
                    );
                    // An action tag handled inline is still an action route.
                    if tagged && self.routes.len() == before {
                        self.push_inline_route(
                            context,
                            depth.max(1),
                            parent,
                            &selectors,
                            relative,
                            arm.pat.span(),
                            &arm_cfg,
                        );
                    }
                }
            }
            other => {
                self.record_dispatch(other, relative, context, depth, parent, inherited, cfg);
            }
        }
    }

    /// Classify one dispatch-position expression into a route, or report it.
    #[allow(clippy::too_many_arguments)]
    fn record_dispatch(
        &mut self,
        expr: &Expr,
        relative: &str,
        context: &str,
        depth: usize,
        parent: Option<&str>,
        selectors: &[Selector],
        cfg: &[String],
    ) {
        match expr {
            // `Ok(())`, `Err(...)`, `?` wrappers and blocks are transparent.
            Expr::Try(inner) => self.record_dispatch(
                &inner.expr,
                relative,
                context,
                depth,
                parent,
                selectors,
                cfg,
            ),
            Expr::Block(block) => self.walk_block(
                &block.block,
                relative,
                context,
                depth,
                parent,
                selectors,
                cfg,
            ),
            Expr::If(_) | Expr::Match(_) => {
                self.walk_expr(expr, relative, context, depth, parent, selectors, cfg);
            }
            Expr::Call(call) => {
                let Expr::Path(path) = call.func.as_ref() else {
                    self.report(context, expr, relative, "call target is not a path");
                    return;
                };
                let target = render_path(&path.path);
                if is_terminal_call(&target) {
                    return;
                }
                let route_id = self.push_route(
                    &target,
                    depth,
                    parent,
                    selectors,
                    relative,
                    expr.span(),
                    cfg,
                );
                // Follow into the handler to find its action tags.
                if let Some(function) = self.index.resolve(&target) {
                    let function = function.clone();
                    self.walk_function(&function, depth + 1, Some(&route_id), &[], cfg);
                }
            }
            Expr::Return(ret) => {
                if let Some(inner) = &ret.expr {
                    self.record_dispatch(inner, relative, context, depth, parent, selectors, cfg);
                }
            }
            // `X::decode(data).map_err(f).and_then(|request| handler(..))` is
            // one dispatch arm naming one handler. Find the handler rather than
            // recording the arm as an anonymous inline body.
            Expr::MethodCall(_) => {
                if let Some(call) = first_handler_call(expr) {
                    self.record_dispatch(&call, relative, context, depth, parent, selectors, cfg);
                }
            }
            // Ordinary in-body work: an inline handler's own statements. These
            // are not dispatch decisions, and the enclosing guarded region has
            // already been recorded as a route by the caller.
            Expr::Path(_)
            | Expr::Lit(_)
            | Expr::Struct(_)
            | Expr::Assign(_)
            | Expr::Binary(_)
            | Expr::Unary(_)
            | Expr::Field(_)
            | Expr::Index(_)
            | Expr::Reference(_)
            | Expr::Tuple(_)
            | Expr::Array(_)
            | Expr::Range(_)
            | Expr::Cast(_)
            | Expr::Paren(_)
            | Expr::Group(_)
            | Expr::Macro(_)
            | Expr::ForLoop(_)
            | Expr::While(_)
            | Expr::Loop(_)
            | Expr::Let(_)
            | Expr::Break(_)
            | Expr::Continue(_)
            | Expr::Closure(_) => {}
            // An `unsafe` block is a lexical scope, not a dispatch decision.
            // Refusing to look through one told the census that
            // `dclutch-claims-proof-sbf`'s entire account-count/replay-width
            // dispatch did not exist, and that Trading's entrypoint forwarded
            // nowhere. Whether a block asserts obligations the compiler cannot
            // check has nothing to do with which route the wire selected.
            Expr::Unsafe(block) => self.walk_block(
                &block.block,
                relative,
                context,
                depth,
                parent,
                selectors,
                cfg,
            ),
            _ => self.report(
                context,
                expr,
                relative,
                "dispatch-position expression was not a recognised handler call",
            ),
        }
    }

    /// The program's own entry: every instruction reaches this, whatever the
    /// discriminant. Its children refine it.
    fn push_entry_route(&mut self, dispatch: &FunctionFact) {
        let id = format!("{}/{}", self.label, dispatch.name);
        if self.routes.iter().any(|route| route.id == id) {
            return;
        }
        self.routes.push(Route {
            id,
            kind: RouteKind::Entry,
            parent: None,
            handler: qualified(&dispatch.module, &dispatch.name),
            selectors: Vec::new(),
            provenance: format!("{}:1", dispatch.relative),
            cfg: Vec::new(),
            admissible_prestates: Vec::new(),
            selected_prestates: Vec::new(),
        });
    }

    /// Record a route whose body is handled inline, with no named handler.
    #[allow(clippy::too_many_arguments)]
    fn push_inline_route(
        &mut self,
        context: &str,
        depth: usize,
        parent: Option<&str>,
        selectors: &[Selector],
        relative: &str,
        span: Span,
        cfg: &[String],
    ) {
        let tag = specific_tag(selectors);
        let handler = if tag.is_empty() {
            format!("{context} (inline)")
        } else {
            format!("{context} (inline: {tag})")
        };
        let kind = if depth == 0 {
            RouteKind::Entry
        } else {
            RouteKind::Action
        };
        let id = if tag.is_empty() {
            format!("{}/{context}", self.label)
        } else {
            format!("{}/{context}#{tag}", self.label)
        };
        if self.routes.iter().any(|route| route.id == id) {
            return;
        }
        self.routes.push(Route {
            id,
            kind,
            parent: parent.map(str::to_owned),
            handler,
            selectors: selectors.to_vec(),
            provenance: at(relative, span),
            cfg: cfg.to_vec(),
            admissible_prestates: Vec::new(),
            selected_prestates: Vec::new(),
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn push_route(
        &mut self,
        handler: &str,
        depth: usize,
        parent: Option<&str>,
        selectors: &[Selector],
        relative: &str,
        span: Span,
        cfg: &[String],
    ) -> String {
        let kind = if depth == 0 {
            RouteKind::Entry
        } else {
            RouteKind::Action
        };
        // The id carries the MOST SPECIFIC discriminant, not the accumulated
        // union: an or-pattern arm of nine tags dispatching into a handler that
        // re-matches one of them is one route, named for the one.
        let tag = specific_tag(selectors);
        let base = format!("{}/{handler}", self.label);
        // An unguarded call is the handler's continuation, not a distinct
        // public route. It is followed but not counted, so the census
        // denominator stays the set of things a client can actually select.
        if !selects_a_route(selectors, depth) {
            if depth == 0 {
                self.declined_entries += 1;
            }
            return parent.map_or(base, str::to_owned);
        }

        let id = if tag.is_empty() {
            base
        } else {
            format!("{base}#{tag}")
        };
        // A route reached twice by different guards keeps the first, and the
        // second guard is folded in so neither disappears.
        if let Some(existing) = self.routes.iter_mut().find(|route| route.id == id) {
            for selector in selectors {
                let rendered = selector.render();
                if !existing
                    .selectors
                    .iter()
                    .any(|held| held.render() == rendered)
                {
                    existing.selectors.push(selector.clone());
                }
            }
            return id;
        }
        self.routes.push(Route {
            id: id.clone(),
            kind,
            parent: parent.map(str::to_owned),
            handler: handler.to_string(),
            selectors: selectors.to_vec(),
            provenance: at(relative, span),
            cfg: cfg.to_vec(),
            admissible_prestates: Vec::new(),
            selected_prestates: Vec::new(),
        });
        id
    }

    fn report(&mut self, context: &str, expr: &Expr, relative: &str, reason: &str) {
        let expression = render(expr);
        let truncated = if expression.len() > 160 {
            format!("{}…", &expression[..160])
        } else {
            expression
        };
        self.unclassified.push(Unclassified {
            context: context.to_string(),
            expression: truncated,
            provenance: at(relative, expr.span()),
            reason: reason.to_string(),
        });
    }

    /// Classify one `match` arm pattern into wire discriminants.
    ///
    /// `Some`/`None`/`Ok`/`Err` are the enclosing decode's own shape, never an
    /// action tag; those arms rely on the arm guard for their discriminant.
    fn arm_selectors(&self, pat: &Pat) -> Vec<Selector> {
        match pat {
            Pat::Wild(_) => vec![Selector::Fallthrough],
            Pat::Or(alternatives) => alternatives
                .cases
                .iter()
                .flat_map(|case| self.arm_selectors(case))
                .collect(),
            Pat::Lit(literal) => vec![Selector::Tag {
                text: render(literal),
            }],
            Pat::TupleStruct(tuple)
                if matches!(
                    render_path(&tuple.path).rsplit("::").next(),
                    Some("Some" | "Ok")
                ) =>
            {
                tuple
                    .elems
                    .iter()
                    .flat_map(|inner| self.arm_selectors(inner))
                    .collect()
            }
            _ => match variant_path(pat) {
                Some(path) => {
                    let name = path.rsplit("::").next().unwrap_or(&path);
                    if matches!(name, "Some" | "None" | "Ok" | "Err") {
                        return Vec::new();
                    }
                    // A `const` pattern is a width or action discriminant, not
                    // an enum variant. Resolve it so the census shows the value;
                    // an unresolvable SCREAMING_CASE name is still not a variant.
                    let screaming = name.chars().all(|character| {
                        character.is_ascii_uppercase()
                            || character == '_'
                            || character.is_ascii_digit()
                    });
                    match self.constants.resolve(name) {
                        Some(fact) => match &fact.value {
                            ConstantValue::Integer(value) => vec![Selector::Length {
                                constant: path.clone(),
                                value: Some(*value),
                                provenance: Some(fact.provenance.clone()),
                            }],
                            ConstantValue::Bytes { hex, ascii } => vec![Selector::Magic {
                                constant: path.clone(),
                                bytes: Some(hex.clone()),
                                ascii: ascii.clone(),
                                provenance: Some(fact.provenance.clone()),
                            }],
                        },
                        None if screaming => vec![Selector::Tag { text: path }],
                        None => vec![Selector::Variant {
                            native: self.native_variant_selector(&path),
                            path,
                        }],
                    }
                }
                None => vec![Selector::Tag { text: render(pat) }],
            },
        }
    }

    /// Resolve the shipped byte selector for action enums whose decoder's
    /// offset is explicitly mapped here.
    ///
    /// The discriminant value itself is not mirrored: it comes from the enum
    /// declaration indexed out of the first-party dependency closure. The
    /// offset comes from the named public codec constant. A family absent from
    /// this closed map remains a path-only variant and therefore cannot gain
    /// native execution credit.
    fn native_variant_selector(&self, path: &str) -> Option<NativeVariantSelector> {
        let enumeration = path.rsplit("::").nth(1)?;
        let offset_name = match enumeration {
            "FractionalExposureActionV2" => "FRACTIONAL_EXPOSURE_REQUEST_ACTION_OFFSET_V2",
            _ => return None,
        };
        let offset = self.constants.resolve(offset_name)?;
        let ConstantValue::Integer(offset_value) = &offset.value else {
            return None;
        };
        let variant = self.predicates.variant(path)?;
        Some(NativeVariantSelector {
            offset: usize::try_from(*offset_value).ok()?,
            value: u8::try_from(variant.value).ok()?,
            offset_provenance: offset.provenance.clone(),
            value_provenance: variant.provenance.clone(),
        })
    }

    /// Pull wire discriminants out of an `if` guard.
    fn selectors_from(&self, cond: &Expr) -> Vec<Selector> {
        let mut found = Vec::new();
        self.scan_condition(cond, &mut found);
        found
    }

    fn scan_condition(&self, expr: &Expr, out: &mut Vec<Selector>) {
        match expr {
            Expr::Binary(binary) => {
                self.scan_condition(&binary.left, out);
                self.scan_condition(&binary.right, out);
                if matches!(binary.op, BinOp::Eq(_) | BinOp::Ne(_))
                    && let Expr::Lit(literal) = binary.right.as_ref()
                    && let syn::Lit::Int(value) = &literal.lit
                {
                    out.push(Selector::Literal {
                        text: format!("{} {}", render(&binary.left), value.base10_digits()),
                    });
                }
            }
            Expr::Unary(unary) => self.scan_condition(&unary.expr, out),
            Expr::Paren(paren) => self.scan_condition(&paren.expr, out),
            Expr::Group(group) => self.scan_condition(&group.expr, out),
            Expr::Reference(reference) => self.scan_condition(&reference.expr, out),
            Expr::Try(inner) => self.scan_condition(&inner.expr, out),
            Expr::MethodCall(call) => {
                self.scan_condition(&call.receiver, out);
                for argument in &call.args {
                    self.scan_condition(argument, out);
                }
            }
            Expr::Call(call) => {
                if let Expr::Path(path) = call.func.as_ref() {
                    let written = render_path(&path.path);
                    let target = self
                        .predicates
                        .canonical_function_path(&written)
                        .to_string();
                    let name = target.rsplit("::").next().unwrap_or(&target);
                    // A guard call is a wire recogniser when its NAME says so
                    // (`is_*`) or when its BODY does -- it states a magic or a
                    // width. The name alone was the rule until 2026-09-07, and
                    // Custody's `upkeep_vault_v1::selects` and
                    // `protocol_parameters_v1::selects` are the two live wires
                    // it could not see: both state an exact width and an
                    // eight-byte magic, both forward to a named handler, and
                    // neither is called `is_`-anything. Reading the body is the
                    // criterion the wire actually has; the name is a habit.
                    let stated = self.recogniser_constants(&target);
                    if name.starts_with("is_") || !stated.is_empty() {
                        // A one-expression equality against one magic is fully
                        // represented by that magic. Keeping the predicate as
                        // a second selector makes the evidence fold reject the
                        // route even though the finalized bytes carry the
                        // complete discriminator. More complex predicates
                        // remain named and therefore fail closed in the fold.
                        if !self.recogniser_is_exact_magic(&target, &stated) {
                            out.push(Selector::Predicate {
                                function: target.clone(),
                            });
                        }
                        for selector in stated {
                            if !out
                                .iter()
                                .any(|existing| existing.render() == selector.render())
                            {
                                out.push(selector);
                            }
                        }
                    }
                }
                for argument in &call.args {
                    self.scan_condition(argument, out);
                }
            }
            Expr::Path(path) => {
                let text = render_path(&path.path);
                let name = text.rsplit("::").next().unwrap_or(&text).to_string();
                self.push_constant_selector(&text, &name, out);
            }
            Expr::Field(field) => self.scan_condition(&field.base, out),
            Expr::Index(index) => {
                self.scan_condition(&index.expr, out);
                self.scan_condition(&index.index, out);
            }
            Expr::Range(range) => {
                if let Some(start) = &range.start {
                    self.scan_condition(start, out);
                }
                if let Some(end) = &range.end {
                    self.scan_condition(end, out);
                }
            }
            _ => {}
        }
    }

    /// The wire constants a guard's recogniser states, one hop deep.
    ///
    /// Empty when the call resolves to nothing, or to a body that names no
    /// magic and no width -- which is also the test [`Self::scan_condition`]
    /// uses to decide whether a guard call is a recogniser at all.
    ///
    /// A predicate is a real selector and naming it was right, but naming it
    /// was ALL the census did, so a route selected by one carried no bytes at
    /// all. Measured 2026-09-04: `DCLTPUA1`, `DCLTSPI1` and `DCLTDFS1` each
    /// executed on devnet across three cohorts and occurred zero times in the
    /// whole inventory, so `corroborate.py --discover` could resolve no
    /// transaction that carried them and the devnet witness count sat at 22
    /// while real routes ran. The routes were never missing; their bytes were.
    /// Trading, whose entire top-level surface is predicate-selected, reported
    /// zero magics -- `DCLTHOT3` included.
    ///
    /// Exactly one hop, and only the constants: the predicate's own body is
    /// scanned for `MAGIC`/width constants and any nested `Predicate` or
    /// `Literal` it finds is dropped. Following further would make an
    /// entrypoint's selector set a function of arbitrary call depth, which is
    /// the ambiguity `index` exists to refuse. A predicate that cannot be
    /// resolved, or that states no constant, leaves the row exactly as it was.
    ///
    /// ONE further hop, and only for a predicate that DELEGATES ENTIRELY.
    /// `is_generic_market_founding_v3` is
    /// `GenericMarketFoundingCallerBumpsV3::decode(instruction_data).is_ok()`
    /// -- it states no constant of its own, hands its whole parameter to one
    /// call, and every byte it recognises (`DCLTGMF3`, and the 13-byte width)
    /// is written inside that callee. This is not arbitrary depth: the
    /// forwarding predicate is a wrapper with no content, and refusing to look
    /// through it left the SOLE Trading route for `DCLTGMF3` with no bytes at
    /// all, so two finalized cohort-15 transactions carrying that magic
    /// resolved to no route. The hop is taken only when the predicate's own
    /// body yielded nothing, and the callee's body is read the same way -- the
    /// constants only.
    fn recogniser_constants(&self, target: &str) -> Vec<Selector> {
        // A crate-qualified call names a CRATE, and a crate root's module path
        // is empty, so `resolve` cannot match `dclutch_x_contract` against it.
        // The bare name is the fallback and it is still cautious: `resolve`
        // refuses two same-named functions rather than guessing between them.
        let resolved = self.predicates.resolve(target).or_else(|| {
            let name = target.rsplit("::").next().unwrap_or(target);
            self.predicates.resolve(name)
        });
        let Some(function) = resolved else {
            return Vec::new();
        };
        let mut found = Vec::new();
        self.scan_function_body(function, &mut found);
        if !found
            .iter()
            .any(|selector| matches!(selector, Selector::Magic { .. } | Selector::Length { .. }))
            && let Some(callee) = forwarded_call(function)
            && let Some(body) = self.predicates.resolve_from(&function.module, &callee)
        {
            self.scan_function_body(body, &mut found);
        }
        let mut stated: Vec<Selector> = Vec::new();
        for selector in found {
            if !matches!(selector, Selector::Magic { .. } | Selector::Length { .. }) {
                continue;
            }
            if !stated
                .iter()
                .any(|existing| existing.render() == selector.render())
            {
                stated.push(selector);
            }
        }
        stated
    }

    /// Whether a predicate's entire body is one equality against one magic.
    ///
    /// This deliberately recognizes only the narrow shape used by selectors
    /// such as `is_dealer_found_v1`: one tail expression, `==`, and one magic
    /// read from that expression. A conjunction, early-return decoder,
    /// forwarding wrapper, length check or extra tag remains a `Predicate`
    /// plus its readable constants, because the persisted selector vector
    /// cannot say whether those facts are alternatives or conjunctions.
    fn recogniser_is_exact_magic(&self, target: &str, stated: &[Selector]) -> bool {
        if stated.len() != 1 || !matches!(stated[0], Selector::Magic { .. }) {
            return false;
        }
        let resolved = self.predicates.resolve(target).or_else(|| {
            let name = target.rsplit("::").next().unwrap_or(target);
            self.predicates.resolve(name)
        });
        let Some(function) = resolved else {
            return false;
        };
        let [Stmt::Expr(Expr::Binary(binary), None)] = function.block.stmts.as_slice() else {
            return false;
        };
        matches!(binary.op, BinOp::Eq(_))
    }

    /// Scan one function body's statements for wire discriminants.
    ///
    /// A recogniser states its bytes in one of two shapes and both are read
    /// here: as the tail expression (`data.len() == N && data[..8] == MAGIC`),
    /// or as an early-return GUARD (`if data.len() != N || data[..8] != MAGIC
    /// { return Err(..) }`). Only the first was read before, which is why a
    /// `decode` -- where every width check in this tree is written as a guard
    /// -- yielded nothing at all. The `if` contributes its CONDITION and
    /// nothing else; its branches are the handler, not the selector.
    fn scan_function_body(&self, function: &FunctionFact, out: &mut Vec<Selector>) {
        for statement in &function.block.stmts {
            match statement {
                Stmt::Expr(expr, _) => self.scan_body_expression(expr, out),
                Stmt::Local(local) => {
                    if let Some(initializer) = &local.init {
                        self.scan_body_expression(&initializer.expr, out);
                    }
                }
                _ => {}
            }
        }
    }

    fn scan_body_expression(&self, expr: &Expr, out: &mut Vec<Selector>) {
        match expr {
            Expr::If(conditional) => {
                self.scan_condition(&conditional.cond, out);
                if let Some((_, otherwise)) = &conditional.else_branch {
                    self.scan_body_expression(otherwise, out);
                }
            }
            // `else { .. }` is a block; an `else if` is the expression above.
            Expr::Block(block) => {
                for statement in &block.block.stmts {
                    if let Stmt::Expr(inner, _) = statement {
                        self.scan_body_expression(inner, out);
                    }
                }
            }
            other => self.scan_condition(other, out),
        }
    }

    fn push_constant_selector(&self, full: &str, name: &str, out: &mut Vec<Selector>) {
        let looks_magic = name.contains("MAGIC");
        let looks_width = name.contains("BYTES") || name.contains("_LEN") || name.contains("COUNT");
        if !looks_magic && !looks_width {
            return;
        }
        let resolved = self.constants.resolve(name);
        let selector = if looks_magic {
            match resolved.map(|fact| (&fact.value, &fact.provenance)) {
                Some((ConstantValue::Bytes { hex, ascii }, provenance)) => Selector::Magic {
                    constant: full.to_string(),
                    bytes: Some(hex.clone()),
                    ascii: ascii.clone(),
                    provenance: Some(provenance.clone()),
                },
                _ => Selector::Magic {
                    constant: full.to_string(),
                    bytes: None,
                    ascii: None,
                    provenance: resolved.map(|fact| fact.provenance.clone()),
                },
            }
        } else {
            match resolved.map(|fact| (&fact.value, &fact.provenance)) {
                Some((ConstantValue::Integer(value), provenance)) => Selector::Length {
                    constant: full.to_string(),
                    value: Some(*value),
                    provenance: Some(provenance.clone()),
                },
                _ => Selector::Length {
                    constant: full.to_string(),
                    value: None,
                    provenance: resolved.map(|fact| fact.provenance.clone()),
                },
            }
        };
        let rendered = selector.render();
        if !out.iter().any(|held| held.render() == rendered) {
            out.push(selector);
        }
    }
}

/// The one call a forwarding predicate hands its whole parameter to.
///
/// `Some` only when the body writes EXACTLY ONE path call taking a bare
/// parameter of the enclosing function -- `Type::decode(instruction_data)`.
/// Two such calls is a predicate that composes rather than delegates, and it
/// gets no hop: which of them states the selector is exactly the question
/// this reader must not answer by guessing.
fn forwarded_call(function: &FunctionFact) -> Option<String> {
    let parameters: BTreeSet<&str> = function.parameters.iter().map(String::as_str).collect();
    let mut found = BTreeSet::new();
    for statement in &function.block.stmts {
        match statement {
            Stmt::Expr(expr, _) => collect_forwarded(expr, &parameters, &mut found),
            Stmt::Local(local) => {
                if let Some(initializer) = &local.init {
                    collect_forwarded(&initializer.expr, &parameters, &mut found);
                }
            }
            _ => {}
        }
    }
    if found.len() == 1 {
        found.into_iter().next()
    } else {
        None
    }
}

fn collect_forwarded(expr: &Expr, parameters: &BTreeSet<&str>, out: &mut BTreeSet<String>) {
    match expr {
        Expr::Call(call) => {
            if let Expr::Path(path) = call.func.as_ref() {
                let target = render_path(&path.path);
                let forwards = call.args.iter().any(|argument| {
                    let argument = match argument {
                        Expr::Reference(reference) => reference.expr.as_ref(),
                        other => other,
                    };
                    matches!(argument, Expr::Path(inner)
                        if parameters.contains(render_path(&inner.path).as_str()))
                });
                if forwards && target.contains("::") {
                    out.insert(target);
                }
            }
            for argument in &call.args {
                collect_forwarded(argument, parameters, out);
            }
        }
        Expr::MethodCall(call) => {
            collect_forwarded(&call.receiver, parameters, out);
            for argument in &call.args {
                collect_forwarded(argument, parameters, out);
            }
        }
        Expr::Paren(inner) => collect_forwarded(&inner.expr, parameters, out),
        Expr::Group(inner) => collect_forwarded(&inner.expr, parameters, out),
        Expr::Unary(inner) => collect_forwarded(&inner.expr, parameters, out),
        Expr::Try(inner) => collect_forwarded(&inner.expr, parameters, out),
        Expr::Reference(inner) => collect_forwarded(&inner.expr, parameters, out),
        Expr::Binary(binary) => {
            collect_forwarded(&binary.left, parameters, out);
            collect_forwarded(&binary.right, parameters, out);
        }
        _ => {}
    }
}

/// A guard term a client can actually control from the wire.
///
/// A bare `Literal` from an `if` condition is an internal postcondition check
/// (`if balance == 0`), never a route selector; `Fallthrough` alone is a route
/// only at the dispatch function itself, where it means "the default
/// instruction shape".
fn is_wire_discriminant(selector: &Selector) -> bool {
    matches!(
        selector,
        Selector::Magic { .. }
            | Selector::Length { .. }
            | Selector::Predicate { .. }
            | Selector::Variant { .. }
            | Selector::Tag { .. }
    )
}

fn selects_a_route(selectors: &[Selector], depth: usize) -> bool {
    selectors.iter().any(is_wire_discriminant)
        || (depth == 0 && selectors.iter().any(|s| matches!(s, Selector::Fallthrough)))
}

/// The first non-terminal function call inside a method-call chain, including
/// closure bodies. Bounded: it stops at the first hit.
fn first_handler_call(expr: &Expr) -> Option<Expr> {
    match expr {
        Expr::Call(call) => {
            if let Expr::Path(path) = call.func.as_ref() {
                let target = render_path(&path.path);
                if !is_terminal_call(&target) {
                    return Some(expr.clone());
                }
            }
            call.args.iter().find_map(first_handler_call)
        }
        Expr::MethodCall(call) => first_handler_call(&call.receiver)
            .or_else(|| call.args.iter().find_map(first_handler_call)),
        Expr::Closure(closure) => first_handler_call(&closure.body),
        Expr::Try(inner) => first_handler_call(&inner.expr),
        Expr::Paren(inner) => first_handler_call(&inner.expr),
        Expr::Group(inner) => first_handler_call(&inner.expr),
        _ => None,
    }
}

fn qualified(module: &str, name: &str) -> String {
    if module.is_empty() {
        name.to_string()
    } else {
        format!("{module}::{name}")
    }
}

/// Whether a `let` initialiser is a DISPATCH, and not a lookup table.
///
/// This arm used to be `Stmt::Local(_) => {}` — every initialiser skipped — and
/// the cost was measured on 2026-09-02: welding Core's `CloseFund` shut moved
/// its refusal into `let action = match resolution_request.action { … }`, and
/// the census silently dropped `core/resolution::process#CloseFund` while the
/// refusal it recorded was still live on chain. 160 routes to 159, that row and
/// no other. **A refusal that still exists is not necessarily a refusal that is
/// still recorded**, and the instrument that made that true was this arm.
///
/// Descending into EVERY initialiser is not the fix and was measured too: 160
/// routes to 246, with 17 previously-classified positions going unclassified.
/// It invents `custody/Pubkey::find_program_address#None` and
/// `core/account#recovery_id` — value lookups keyed by a tag, which is most of
/// the 222 `let … = match …` sites this tree contains. A route is a shape a
/// CLIENT can select, and the census's denominator stops meaning that if every
/// table indexed by an action becomes a row.
///
/// The line between them is DIVERGENCE, and it is a fact about the code rather
/// than a heuristic about names. A guard decides whether the instruction may
/// proceed, so at least one of its branches leaves the function. A table
/// produces a value for every branch and leaves none. That is exactly the
/// difference between the `CloseFund` arm that returns `UnsupportedAction` and
/// the `expected_writable` arm four hundred lines below it that returns
/// `[true, false]`.
fn local_initialiser_dispatches(expr: &Expr) -> bool {
    match expr {
        Expr::Match(matched) => matched.arms.iter().any(|arm| diverges(&arm.body)),
        Expr::If(branch) => {
            branch.then_branch.stmts.iter().any(statement_diverges)
                || branch
                    .else_branch
                    .as_ref()
                    .is_some_and(|(_, otherwise)| diverges(otherwise))
        }
        _ => false,
    }
}

/// Whether an expression leaves the enclosing function.
///
/// `return` only. `?` is deliberately NOT divergence here: a fallible call in
/// a table arm (`let (a, b) = match action { A => f()?, … }`) is a lookup that
/// can fail, not a decision about which instruction shape ran, and admitting it
/// would put the table rows back.
fn diverges(expr: &Expr) -> bool {
    match expr {
        Expr::Return(_) => true,
        Expr::Block(block) => block.block.stmts.iter().any(statement_diverges),
        _ => false,
    }
}

fn statement_diverges(statement: &Stmt) -> bool {
    match statement {
        Stmt::Expr(expr, _) => diverges(expr),
        _ => false,
    }
}

fn is_control(expr: &Expr) -> bool {
    matches!(expr, Expr::If(_) | Expr::Match(_) | Expr::Return(_))
}

/// Calls that end a dispatch chain without naming a handler.
fn is_terminal_call(target: &str) -> bool {
    let name = target.rsplit("::").next().unwrap_or(target);
    matches!(
        name,
        // Constructors and wrappers.
        "Ok" | "Err" | "Some" | "None" | "from" | "into" | "new"
        // Decoders: `X::decode(data).and_then(|r| handler(r))` names the
        // handler, not the decode.
        | "decode" | "try_from" | "parse" | "from_bytes"
    )
}

fn variant_path(pat: &Pat) -> Option<String> {
    match pat {
        Pat::Path(path) => Some(render_path(&path.path)),
        Pat::TupleStruct(tuple) => Some(render_path(&tuple.path)),
        Pat::Struct(structure) => Some(render_path(&structure.path)),
        // `A | B` selects one route through two tags; both are recorded.
        Pat::Or(alternatives) => {
            let variants: Vec<String> =
                alternatives.cases.iter().filter_map(variant_path).collect();
            if variants.is_empty() {
                None
            } else {
                Some(variants.join("|"))
            }
        }
        Pat::Ident(ident) if ident.subpat.is_none() => None,
        _ => None,
    }
}

// -------------------------------------------------------------------- driver

pub struct ProgramTarget {
    pub package: String,
    pub label: String,
}

/// Enumerate every program's dispatch surface under `root`.
pub fn enumerate(
    root: &Path,
    sources: &Sources,
    targets: &[ProgramTarget],
    constants: &ConstantIndex,
    admissions: &AdmissionIndex,
    source_revision: Option<String>,
) -> Result<Inventory, String> {
    let mut programs = Vec::new();
    let collisions = admissions.collisions();
    for target in targets {
        let crate_src = root.join("programs").join(&target.package).join("src");
        let lib = crate_src.join("lib.rs");
        if !lib.is_file() {
            return Err(format!("no crate root at {}", lib.display()));
        }
        let index = index_crate(sources, &crate_src);
        // The guard descent reads a wider namespace than the dispatch walk:
        // this program's own crate first, then every first-party crate it
        // depends on. `resolve_from` prefers the caller's own module, so a
        // name a program and a codec share still resolves to the one the
        // compiler picks.
        let mut guard_sources = vec![crate_src.clone()];
        guard_sources.extend(first_party_dependencies(
            root,
            &root
                .join("programs")
                .join(&target.package)
                .join("Cargo.toml"),
        ));
        let guard_index = index_sources(sources, &guard_sources);
        let text =
            fs::read_to_string(&lib).map_err(|error| format!("read {}: {error}", lib.display()))?;
        let file: File =
            syn::parse_file(&text).map_err(|error| format!("parse {}: {error}", lib.display()))?;
        let lib_relative = relative(root, &lib);

        let mut refusals = Vec::new();
        for source in sources.under(&crate_src) {
            collect_refusals(
                &source.file.items,
                &target.label,
                &source.relative,
                &mut refusals,
            );
        }
        refusals.sort_by(|left, right| left.id.cmp(&right.id));
        refusals.dedup_by(|left, right| left.id == right.id);

        let mut walk = DispatchWalk {
            label: &target.label,
            index: &index,
            predicates: &guard_index,
            constants,
            routes: Vec::new(),
            unclassified: Vec::new(),
            visited: BTreeSet::new(),
            declined_entries: 0,
        };

        // The entrypoint does not have to live in `lib.rs`. `9abed0c` moved
        // Trading's SBF entrypoint, its loader-input deserializer, and its
        // allocator into a named machine-boundary module, and a scan restricted
        // to the crate root then reported that the whole program "exposes no
        // dispatch surface" - which silently dropped every Trading route from
        // this census while W1f was executing two of them on a validator. A
        // program's public entry is a fact about the crate, not about one file.
        let mut discovered = find_entrypoints(&file.items, &lib_relative);
        for source in sources.under(&crate_src) {
            if source.path == lib {
                continue;
            }
            discovered.extend(find_entrypoints(&source.file.items, &source.relative));
        }
        discovered.dedup_by(|left, right| left.1 == right.1);

        let mut entrypoints = Vec::new();
        for (macro_name, function_name, provenance) in discovered {
            let resolved = index.resolve(&function_name).is_some();
            entrypoints.push(Entrypoint {
                macro_name,
                function: function_name.clone(),
                provenance,
                resolved,
            });
            let Some(function) = index.resolve(&function_name) else {
                walk.unclassified.push(Unclassified {
                    context: "entrypoint".into(),
                    expression: function_name.clone(),
                    provenance: lib_relative.clone(),
                    reason: "entrypoint function was not found in this crate; \
                             its dispatch surface is NOT enumerated"
                        .into(),
                });
                continue;
            };
            let function = function.clone();
            // The entrypoint shim usually forwards straight to
            // `process_instruction`. Walking it at depth 0 makes that forward a
            // route; unwrap it so the real dispatch branches are the entries.
            let dispatch = unwrap_forwarding_shim(&index, function);
            // The entrypoint itself is always a route. A program whose dispatch
            // has no internal branch structure still has exactly one public
            // entry, and the census must be able to say whether it has ever
            // executed rather than showing an empty table.
            walk.push_entry_route(&dispatch);
            walk.walk_function(&dispatch, 0, None, &[], &[]);
        }

        let mut routes = walk.routes;
        routes.sort_by(|left, right| left.id.cmp(&right.id));

        // A route's phase gate is read from the constants its OWN guards check
        // against, following its handler's calls and stopping at the next
        // route boundary. A route the walk finds no constant for carries none,
        // and the census says exactly that rather than inferring one.
        let mut guards = GuardMap::new(admissions, &guard_index, &routes);
        type Attributed = Vec<crate::model::PhaseAdmission>;
        let attributed: Vec<(Attributed, Attributed)> =
            routes.iter().map(|route| guards.for_route(route)).collect();
        for (route, (admissible, selected)) in routes.iter_mut().zip(attributed) {
            route.admissible_prestates = admissible;
            route.selected_prestates = selected;
        }

        let mut unclassified = walk.unclassified;
        // A constant of the admission type written in a shape the reader does
        // not understand is reported here rather than dropped: an enumerator
        // that silently under-counts is the mirror failure one level up.
        let package_prefix = format!("programs/{}/", target.package);
        unclassified.extend(
            admissions
                .unreadable
                .iter()
                .chain(collisions.iter())
                .filter(|entry| entry.provenance.starts_with(&package_prefix))
                .cloned(),
        );
        // A program with no persisted lifecycle discriminant says so as a
        // column value; a declaration the sources refute becomes unclassified
        // in the same run rather than outliving the state model.
        let declared_constants = routes
            .iter()
            .flat_map(|route| route.admissible_prestates.iter())
            .count();
        let no_persisted_discriminant = match crate::phases::no_persisted_discriminant(
            &target.label,
            &crate_src,
            root,
            declared_constants,
        ) {
            Ok(reason) => reason,
            Err(stale) => {
                unclassified.push(stale);
                None
            }
        };
        unclassified.sort_by(|left, right| left.provenance.cmp(&right.provenance));

        programs.push(ProgramSurface {
            package: target.package.clone(),
            label: target.label.clone(),
            crate_root: lib_relative,
            entrypoints,
            routes,
            refusals,
            unclassified,
            no_persisted_discriminant,
        });
    }

    Ok(Inventory {
        schema: crate::model::INVENTORY_SCHEMA_V1.into(),
        source_root: root.to_string_lossy().into_owned(),
        source_revision,
        programs,
        bands: Vec::new(),
    })
}

/// How many forwarding hops the enumerator will unwrap before the entrypoint.
///
/// Three is what the deepest real shim needs (`entrypoint` -> width arm ->
/// `dispatch` -> `process_instruction`); the bound exists so a cycle the
/// resolver cannot see cannot loop here.
const MAX_FORWARD_HOPS: usize = 4;

/// Unwrap a machine-boundary shim down to the function it dispatches from.
///
/// A shim is not a route. `dclutch-trading-sbf` deserializes the loader's input
/// region itself so that up to `ADAPTER_STACK_SLOTS_V1` accounts cost no heap,
/// which puts three functions between the loader symbol and
/// `process_instruction`: `entrypoint` branches on the account count,
/// `entrypoint_on_stack`/`entrypoint_on_heap` deserialize, and `dispatch` lifts
/// the heap ceiling. Walking from the loader symbol spends the whole
/// `MAX_DISPATCH_DEPTH` budget on those three and enumerates none of the routes
/// underneath, so the census reported one route for a program with five.
///
/// Two shapes are unwrapped, and neither is a dispatch decision:
///
/// - one dispatch-position call, however many `let`s, `unsafe` blocks or
///   `match`-on-the-result wrappers surround it; and
/// - a fan-out whose every branch reconverges on the same single function. The
///   branch selected a physical frame shape, not a route: both arms run the
///   identical program. Recording the arms as routes would be strictly worse
///   than saying nothing, because it would name two ids for one wire surface
///   and push the real routes past the depth budget.
///
/// A function whose branches do NOT reconverge is a dispatcher and is where
/// unwrapping stops.
fn unwrap_forwarding_shim(index: &CrateIndex, start: FunctionFact) -> FunctionFact {
    let mut current = start;
    for _ in 0..MAX_FORWARD_HOPS {
        let Some(name) = forwarding_target(index, &current) else {
            break;
        };
        let Some(next) = index.resolve(&name) else {
            break;
        };
        if next.name == current.name && next.module == current.module {
            break;
        }
        current = next.clone();
    }
    current
}

/// The one function every dispatch path in `function` reaches, if there is one.
fn forwarding_target(index: &CrateIndex, function: &FunctionFact) -> Option<String> {
    // A body that is exactly one call is a shim wherever it appears: this is
    // the rule that has always unwrapped `entrypoint!(process_instruction)`
    // shims, and it stays unconditional.
    if let Some(single) = single_forward(&function.block) {
        return Some(single);
    }
    // Everything below is loader plumbing only. Without the gate the rules
    // would also unwrap `dclutch-dealer-sbf`'s single-route body into whichever
    // helper it finishes with, renaming a route the ledger already names.
    if !function.machine_boundary {
        return None;
    }
    if let Some(sole) = sole_forward_target(index, function) {
        return Some(sole);
    }
    let branches = resolvable_forward_targets(index, &function.block);
    if branches.len() < 2 {
        return None;
    }
    let mut common: Option<String> = None;
    for branch in &branches {
        let inner = index.resolve(branch)?;
        if !inner.machine_boundary {
            return None;
        }
        let reached = sole_forward_target(index, inner)?;
        match &common {
            None => common = Some(reached),
            Some(seen) if seen == &reached => {}
            Some(_) => return None,
        }
    }
    common
}

/// If a block is exactly one call `f(a, b, c)`, return `f`.
fn single_forward(block: &Block) -> Option<String> {
    if block.stmts.len() != 1 {
        return None;
    }
    let Stmt::Expr(Expr::Call(call), _) = &block.stmts[0] else {
        return None;
    };
    let Expr::Path(path) = call.func.as_ref() else {
        return None;
    };
    Some(render_path(&path.path))
}

/// The single function a body forwards to, or `None` if it forwards to none or
/// to several.
fn sole_forward_target(index: &CrateIndex, function: &FunctionFact) -> Option<String> {
    let mut targets = resolvable_forward_targets(index, &function.block);
    if targets.len() == 1 {
        Some(targets.remove(0))
    } else {
        None
    }
}

/// Every dispatch-position call in a body that resolves to a function of this
/// crate, deduplicated. Terminal wrappers (`Ok`, `u64::from`, `X::decode`) are
/// not forwards and are excluded by the same rule the walker uses.
fn resolvable_forward_targets(index: &CrateIndex, block: &Block) -> Vec<String> {
    let mut targets = Vec::new();
    collect_forward_targets(block, &mut targets);
    targets.retain(|target| index.resolve(target).is_some());
    targets.sort();
    targets.dedup();
    targets
}

fn collect_forward_targets(block: &Block, out: &mut Vec<String>) {
    let count = block.stmts.len();
    for (position, statement) in block.stmts.iter().enumerate() {
        if let Stmt::Expr(expr, semi) = statement {
            if position + 1 == count || semi.is_none() || is_control(expr) {
                collect_forward_expr(expr, out);
            }
        }
    }
}

fn collect_forward_expr(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::Unsafe(block) => collect_forward_targets(&block.block, out),
        Expr::Block(block) => collect_forward_targets(&block.block, out),
        Expr::Paren(inner) => collect_forward_expr(&inner.expr, out),
        Expr::Group(inner) => collect_forward_expr(&inner.expr, out),
        Expr::Try(inner) => collect_forward_expr(&inner.expr, out),
        Expr::Return(ret) => {
            if let Some(inner) = &ret.expr {
                collect_forward_expr(inner, out);
            }
        }
        Expr::If(branch) => {
            collect_forward_targets(&branch.then_branch, out);
            if let Some((_, otherwise)) = &branch.else_branch {
                collect_forward_expr(otherwise, out);
            }
        }
        // `match f(x) { Ok(..) => .., Err(e) => .. }` forwards to `f` when the
        // arms only re-wrap its result; the arms are checked first so a real
        // dispatch on a decoded discriminant is never mistaken for a forward.
        Expr::Match(matched) => {
            let before = out.len();
            for arm in &matched.arms {
                collect_forward_expr(&arm.body, out);
            }
            if out.len() == before {
                collect_forward_expr(&matched.expr, out);
            }
        }
        Expr::Call(call) => {
            if let Expr::Path(path) = call.func.as_ref() {
                let target = render_path(&path.path);
                if !is_terminal_call(&target) {
                    out.push(target);
                }
            }
        }
        _ => {}
    }
}

/// A short, line-stable fingerprint of a route's most specific wire
/// discriminant. Route ids must survive an unrelated edit moving the source
/// line, so ids are built from selector names rather than positions.
fn specific_tag(selectors: &[Selector]) -> String {
    let mut last = None;
    let mut fallthrough = false;
    for selector in selectors {
        match selector {
            Selector::Variant { path, .. } => {
                last = Some(path.rsplit("::").next().unwrap_or(path).to_string());
            }
            Selector::Tag { text } => last = Some(text.replace(' ', "")),
            Selector::Fallthrough => fallthrough = true,
            // A magic or length already names the handler uniquely; adding it
            // to the id would only make the id longer, not more distinguishing.
            Selector::Magic { .. }
            | Selector::Length { .. }
            | Selector::Predicate { .. }
            | Selector::Literal { .. } => {}
        }
    }
    match (last, fallthrough) {
        (Some(tag), _) => tag,
        (None, true) => "else".to_string(),
        (None, false) => String::new(),
    }
}

#[cfg(test)]
mod native_variant_tests {
    use super::{
        ConstantFact, ConstantIndex, ConstantValue, DispatchWalk, Selector, collect_imports,
        fold_pending_constants, index_constants_in_items, index_source,
    };
    use crate::ledger::{
        CLAIM_CHECK_V1_ACTION_OFFSET, CLAIM_CHECK_V1_AGGREGATE, CLAIM_CHECK_V1_CLOSE_BODY_RESERVED,
        CLAIM_CHECK_V1_CLOSE_ESCROW_ACTION, CLAIM_CHECK_V1_CLOSE_ESCROW_BYTES,
        CLAIM_CHECK_V1_HEADER_RESERVED, CLAIM_CHECK_V1_MAGIC, CLAIM_CHECK_V1_REDEEM_ACTION,
        CLAIM_CHECK_V1_REDEEM_BODY_RESERVED, CLAIM_CHECK_V1_REDEEM_BYTES,
        CLAIM_CHECK_V1_REDEEM_OWNER, CLAIM_CHECK_V1_SCHEMA_VERSION,
        CLAIMS_FRACTIONAL_V2_ACTION_OFFSET, CLAIMS_FRACTIONAL_V2_HEADER_RESERVED,
        CLAIMS_FRACTIONAL_V2_MAGIC, CLAIMS_FRACTIONAL_V2_REQUEST_BYTES,
        CLAIMS_FRACTIONAL_V2_SCHEMA_VERSION, CLAIMS_FRACTIONAL_V2_TAIL_RESERVED,
    };
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn fractional_variants_carry_source_derived_action_bytes() {
        let mut facts = BTreeMap::new();
        facts.insert(
            "FRACTIONAL_EXPOSURE_REQUEST_ACTION_OFFSET_V2".into(),
            vec![ConstantFact {
                value: ConstantValue::Integer(10),
                provenance: "crates/dclutch-claims/src/fractional/request_v2.rs:23".into(),
                krate: "dclutch_claims".into(),
            }],
        );
        let constants = ConstantIndex {
            facts,
            crates: BTreeSet::new(),
        };
        let index = index_source(
            "fractional::request_v2",
            r#"
                #[repr(u8)]
                enum FractionalExposureActionV2 {
                    Wrap = 0,
                    WholeUnwrap = 2,
                    TerminalRedeem = 3,
                    TerminalZeroBurn = 4,
                }
            "#,
        );
        let walk = DispatchWalk {
            label: "claims",
            index: &index,
            predicates: &index,
            constants: &constants,
            routes: Vec::new(),
            unclassified: Vec::new(),
            visited: BTreeSet::new(),
            declined_entries: 0,
        };
        let arm: syn::Arm = syn::parse_quote!(
            FractionalExposureActionV2::Wrap
                | FractionalExposureActionV2::WholeUnwrap => Ok(())
        );
        let selectors = walk.arm_selectors(&arm.pat);
        let native: Vec<(&str, usize, u8)> = selectors
            .iter()
            .filter_map(|selector| match selector {
                Selector::Variant {
                    path,
                    native: Some(native),
                } => Some((path.as_str(), native.offset, native.value)),
                _ => None,
            })
            .collect();
        assert_eq!(
            native,
            vec![
                ("FractionalExposureActionV2::Wrap", 10, 0),
                ("FractionalExposureActionV2::WholeUnwrap", 10, 2),
            ]
        );
    }

    #[test]
    fn fractional_native_envelope_is_pinned_to_the_shipped_codec() {
        const RELATIVE: &str = "crates/dclutch-claims/src/fractional/request_v2.rs";
        let source = include_str!("../../../../crates/dclutch-claims/src/fractional/request_v2.rs");
        let file = syn::parse_file(source).expect("shipped Fractional V2 codec parses");
        let mut constants = ConstantIndex::default();
        constants.crates.insert("dclutch_claims".into());
        let mut imports = BTreeMap::new();
        collect_imports(&file.items, &mut imports);
        let mut pending = Vec::new();
        index_constants_in_items(
            &file.items,
            RELATIVE,
            "dclutch_claims",
            &imports,
            &mut constants,
            &mut pending,
        );
        fold_pending_constants(&mut constants, pending);

        let integer = |name: &str| match &constants.resolve(name).expect(name).value {
            ConstantValue::Integer(value) => usize::try_from(*value).expect(name),
            ConstantValue::Bytes { .. } => panic!("{name} is not an integer"),
        };
        let magic = match &constants
            .resolve("FRACTIONAL_EXPOSURE_REQUEST_MAGIC_V2")
            .expect("fractional magic")
            .value
        {
            ConstantValue::Bytes { hex, .. } => hex.clone(),
            ConstantValue::Integer(_) => panic!("fractional magic is not bytes"),
        };

        let native_magic: String = CLAIMS_FRACTIONAL_V2_MAGIC
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(native_magic, magic);
        assert_eq!(
            usize::from(CLAIMS_FRACTIONAL_V2_SCHEMA_VERSION),
            integer("VERSION_V2")
        );
        assert_eq!(
            CLAIMS_FRACTIONAL_V2_ACTION_OFFSET,
            integer("FRACTIONAL_EXPOSURE_REQUEST_ACTION_OFFSET_V2")
        );
        assert_eq!(
            CLAIMS_FRACTIONAL_V2_REQUEST_BYTES,
            integer("FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2")
        );
        assert_eq!(
            CLAIMS_FRACTIONAL_V2_HEADER_RESERVED.start,
            integer("FRACTIONAL_EXPOSURE_REQUEST_HEADER_RESERVED_OFFSET_V2")
        );
        assert_eq!(
            CLAIMS_FRACTIONAL_V2_HEADER_RESERVED.end,
            integer("FRACTIONAL_EXPOSURE_REQUEST_RELEASE_SET_OFFSET_V2")
        );
        assert_eq!(
            CLAIMS_FRACTIONAL_V2_TAIL_RESERVED.start,
            integer("FRACTIONAL_EXPOSURE_REQUEST_TAIL_RESERVED_OFFSET_V2")
        );
        assert_eq!(
            CLAIMS_FRACTIONAL_V2_TAIL_RESERVED.end,
            integer("FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2")
        );
    }

    #[test]
    fn claim_check_native_envelopes_are_pinned_to_the_shipped_codec() {
        const RELATIVE: &str = "crates/dclutch-claims/src/claim_check_request_v1.rs";
        const BASE_RELATIVE: &str = "crates/dclutch-claims/src/claim_check_v1.rs";
        let source =
            include_str!("../../../../crates/dclutch-claims/src/claim_check_request_v1.rs");
        let base_source = include_str!("../../../../crates/dclutch-claims/src/claim_check_v1.rs");
        let file = syn::parse_file(source).expect("shipped ClaimCheck V1 codec parses");
        let base_file = syn::parse_file(base_source).expect("shipped ClaimCheck V1 base parses");
        let mut constants = ConstantIndex::default();
        constants.crates.insert("dclutch_claims".into());
        let mut base_imports = BTreeMap::new();
        collect_imports(&base_file.items, &mut base_imports);
        let mut pending = Vec::new();
        index_constants_in_items(
            &base_file.items,
            BASE_RELATIVE,
            "dclutch_claims",
            &base_imports,
            &mut constants,
            &mut pending,
        );
        let mut imports = BTreeMap::new();
        collect_imports(&file.items, &mut imports);
        index_constants_in_items(
            &file.items,
            RELATIVE,
            "dclutch_claims",
            &imports,
            &mut constants,
            &mut pending,
        );
        fold_pending_constants(&mut constants, pending);

        let integer = |name: &str| match &constants.resolve(name).expect(name).value {
            ConstantValue::Integer(value) => usize::try_from(*value).expect(name),
            ConstantValue::Bytes { .. } => panic!("{name} is not an integer"),
        };
        let magic = match &constants
            .resolve("CLAIM_CHECK_REDEEM_MAGIC_V1")
            .expect("claim-check redeem magic")
            .value
        {
            ConstantValue::Bytes { hex, .. } => hex.clone(),
            ConstantValue::Integer(_) => panic!("claim-check magic is not bytes"),
        };
        let native_magic: String = CLAIM_CHECK_V1_MAGIC
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(native_magic, magic);
        assert_eq!(
            usize::from(CLAIM_CHECK_V1_SCHEMA_VERSION),
            integer("CLAIM_CHECK_WIRE_VERSION_V1")
        );
        assert_eq!(CLAIM_CHECK_V1_ACTION_OFFSET, integer("ACTION_OFFSET"));
        assert_eq!(
            CLAIM_CHECK_V1_HEADER_RESERVED.start,
            integer("RESERVED_HEADER_OFFSET")
        );
        assert_eq!(
            CLAIM_CHECK_V1_HEADER_RESERVED.end,
            integer("REDEEM_AGGREGATE_OFFSET")
        );
        assert_eq!(
            CLAIM_CHECK_V1_AGGREGATE.start,
            integer("REDEEM_AGGREGATE_OFFSET")
        );
        assert_eq!(CLAIM_CHECK_V1_AGGREGATE.end, integer("REDEEM_OWNER_OFFSET"));
        assert_eq!(
            CLAIM_CHECK_V1_REDEEM_OWNER,
            integer("REDEEM_OWNER_OFFSET")..integer("REDEEM_RESERVED_BODY_OFFSET")
        );
        assert_eq!(
            CLAIM_CHECK_V1_REDEEM_BODY_RESERVED,
            integer("REDEEM_RESERVED_BODY_OFFSET")..integer("REDEEM_CLAIM_CHECK_BYTES_V1")
        );
        assert_eq!(
            CLAIM_CHECK_V1_CLOSE_BODY_RESERVED,
            integer("CLOSE_RESERVED_BODY_OFFSET")..integer("CLOSE_CLAIM_CHECK_ESCROW_BYTES_V1")
        );
        assert_eq!(
            CLAIM_CHECK_V1_REDEEM_BYTES,
            integer("REDEEM_CLAIM_CHECK_BYTES_V1")
        );
        assert_eq!(
            CLAIM_CHECK_V1_CLOSE_ESCROW_BYTES,
            integer("CLOSE_CLAIM_CHECK_ESCROW_BYTES_V1")
        );
        let index = index_source("claim_check_request_v1", source);
        assert_eq!(
            index
                .variant("ClaimCheckActionV1::Redeem")
                .expect("Redeem discriminant")
                .value,
            i64::from(CLAIM_CHECK_V1_REDEEM_ACTION)
        );
        assert_eq!(
            index
                .variant("ClaimCheckActionV1::CloseEscrow")
                .expect("CloseEscrow discriminant")
                .value,
            i64::from(CLAIM_CHECK_V1_CLOSE_ESCROW_ACTION)
        );
    }
}

#[cfg(test)]
mod predicate_body_tests {
    use super::{
        ConstantFact, ConstantIndex, ConstantValue, CrateIndex, DispatchWalk, Selector,
        index_source,
    };
    use std::collections::{BTreeMap, BTreeSet};

    /// A route selected by `is_x(data)` used to carry the predicate's NAME and
    /// nothing else, so its wire bytes were absent from the whole inventory
    /// even though the constant is one unambiguous declaration away.
    fn selectors_for(guard: &str, predicate_source: &str, magic: Option<&str>) -> Vec<String> {
        let mut facts: BTreeMap<String, Vec<ConstantFact>> = BTreeMap::new();
        if let Some(ascii) = magic {
            facts.insert(
                "EXAMPLE_MAGIC_V1".to_string(),
                vec![ConstantFact {
                    value: ConstantValue::Bytes {
                        hex: hex_of(ascii),
                        ascii: Some(ascii.to_string()),
                    },
                    provenance: "crates/example/src/lib.rs:1".to_string(),
                    krate: "example".to_string(),
                }],
            );
        }
        let constants = ConstantIndex {
            facts,
            crates: BTreeSet::new(),
        };
        let predicates = index_source("", predicate_source);
        let index = CrateIndex::default();
        let walk = DispatchWalk {
            label: "trading",
            index: &index,
            predicates: &predicates,
            constants: &constants,
            routes: Vec::new(),
            unclassified: Vec::new(),
            visited: BTreeSet::new(),
            declined_entries: 0,
        };
        let condition: syn::Expr = syn::parse_str(guard).expect("guard parses");
        walk.selectors_from(&condition)
            .iter()
            .map(Selector::render)
            .collect()
    }

    fn hex_of(ascii: &str) -> String {
        ascii.bytes().map(|byte| format!("{byte:02x}")).collect()
    }

    const PREDICATE_SOURCE: &str = r#"
        pub fn is_example_v1(input: &[u8]) -> bool {
            input.get(..8) == Some(EXAMPLE_MAGIC_V1.as_slice())
        }
    "#;

    const COMPOUND_PREDICATE_SOURCE: &str = r#"
        pub fn is_example_v1(input: &[u8]) -> bool {
            input.get(..8) == Some(EXAMPLE_MAGIC_V1.as_slice()) && input.len() > 8
        }
    "#;

    /// The shape `DCLTGMF3` is written in: a predicate that delegates its
    /// whole parameter to a `decode`, which states the bytes in a GUARD.
    const FORWARDING_SOURCE: &str = r#"
        struct BumpsV3 { values: [u8; 5] }

        impl BumpsV3 {
            fn decode(instruction_data: &[u8]) -> Result<Self, ProgramError> {
                if instruction_data.len() != EXAMPLE_BYTES_V1
                    || instruction_data.get(..8) != Some(EXAMPLE_MAGIC_V1.as_slice())
                {
                    return Err(Error::Unsupported.into());
                }
                Ok(Self { values: [0; 5] })
            }
        }

        pub fn is_example_v1(instruction_data: &[u8]) -> bool {
            BumpsV3::decode(instruction_data).is_ok()
        }
    "#;

    /// THE `DCLTGMF3` DEFECT. A forwarding predicate is a wrapper with no
    /// content of its own, and stopping at it left the sole Trading route for
    /// that magic with no bytes in the whole inventory -- so two finalized
    /// cohort-15 transactions carrying it resolved to no route at all.
    #[test]
    fn a_forwarding_predicate_carries_the_bytes_its_callee_guards_on() {
        let rendered = selectors_for(
            "generic_market_founding_v1::is_example_v1(instruction_data)",
            FORWARDING_SOURCE,
            Some("DCLTGMF3"),
        );
        assert!(
            rendered
                .iter()
                .any(|text| text == "magic EXAMPLE_MAGIC_V1 = b\"DCLTGMF3\""),
            "the callee's magic must reach the row: {rendered:?}"
        );
    }

    #[test]
    fn an_exact_magic_predicate_collapses_to_its_complete_native_selector() {
        let rendered = selectors_for(
            "dclutch_example_contract::is_example_v1(instruction_data)",
            PREDICATE_SOURCE,
            Some("DCLTEXA1"),
        );
        assert_eq!(
            rendered.len(),
            1,
            "the magic completely describes this predicate"
        );
        assert!(
            rendered
                .iter()
                .any(|text| text.contains("magic EXAMPLE_MAGIC_V1") && text.contains("DCLTEXA1")),
            "the predicate's own bytes must reach the row: {rendered:?}"
        );
    }

    #[test]
    fn a_compound_predicate_keeps_its_unreadable_part() {
        let rendered = selectors_for(
            "dclutch_example_contract::is_example_v1(instruction_data)",
            COMPOUND_PREDICATE_SOURCE,
            Some("DCLTEXA1"),
        );
        assert!(
            rendered
                .iter()
                .any(|text| text == "predicate dclutch_example_contract::is_example_v1()"),
            "the extra condition must remain visible and fail closed: {rendered:?}"
        );
        assert!(
            rendered
                .iter()
                .any(|text| text.contains("magic EXAMPLE_MAGIC_V1") && text.contains("DCLTEXA1")),
            "the readable magic remains available for reports: {rendered:?}"
        );
    }

    /// The hop is exactly one deep and takes only constants: a predicate that
    /// calls another predicate contributes no second `Predicate` selector, so
    /// a row's selector set never becomes a function of call depth.
    #[test]
    fn the_hop_is_one_deep_and_carries_no_nested_predicate() {
        let source = r#"
            pub fn is_outer_v1(input: &[u8]) -> bool {
                is_inner_v1(input) && input.get(..8) == Some(EXAMPLE_MAGIC_V1.as_slice())
            }
            pub fn is_inner_v1(input: &[u8]) -> bool {
                input.len() == 8
            }
        "#;
        let rendered = selectors_for("is_outer_v1(instruction_data)", source, Some("DCLTEXA1"));
        assert_eq!(
            rendered
                .iter()
                .filter(|text| text.starts_with("predicate "))
                .count(),
            1,
            "only the guard's own predicate may be named: {rendered:?}"
        );
    }

    /// An unresolvable predicate leaves the row exactly as it was, which is
    /// the property that makes this change incapable of losing a route.
    #[test]
    fn an_unresolvable_predicate_leaves_the_row_unchanged() {
        let rendered = selectors_for(
            "somewhere::is_not_indexed_v1(instruction_data)",
            "pub fn unrelated() {}",
            Some("DCLTEXA1"),
        );
        assert_eq!(
            rendered,
            vec!["predicate somewhere::is_not_indexed_v1()".to_string()],
        );
    }

    /// A magic the constant index cannot resolve is still reported, unresolved,
    /// rather than dropped -- the same rule an in-guard constant already had.
    #[test]
    fn an_unresolved_constant_is_reported_rather_than_dropped() {
        let rendered = selectors_for(
            "dclutch_example_contract::is_example_v1(instruction_data)",
            PREDICATE_SOURCE,
            None,
        );
        assert!(
            rendered
                .iter()
                .any(|text| text.contains("magic EXAMPLE_MAGIC_V1")),
            "an unresolved magic is still a named selector: {rendered:?}"
        );
    }

    /// A program module may publicly re-export the codec predicate it calls.
    ///
    /// The function body still comes from the codec, and the selector must
    /// carry that canonical public path. Keeping the adapter spelling made a
    /// source consumer search the adapter file for a function declaration
    /// that was correctly absent there.
    #[test]
    fn a_reexported_predicate_carries_its_declared_function_path() {
        let source = r#"
            mod retirement_v1 {
                pub fn is_example_v1(input: &[u8]) -> bool {
                    input.len() > 8
                        && input.get(..8) == Some(EXAMPLE_MAGIC_V1.as_slice())
                }
            }
            mod adapter {
                pub use dclutch_trading::retirement_v1::is_example_v1;
            }
        "#;
        let rendered = selectors_for(
            "adapter::is_example_v1(instruction_data)",
            source,
            Some("DCLTDCU1"),
        );
        assert!(
            rendered.iter().any(|text| {
                text == "predicate dclutch_trading::retirement_v1::is_example_v1()"
            }),
            "the public target, not its declaration-free adapter, identifies the predicate: \
             {rendered:?}"
        );
        assert!(
            rendered
                .iter()
                .any(|text| text == "magic EXAMPLE_MAGIC_V1 = b\"DCLTDCU1\""),
            "the re-exported body still supplies its magic: {rendered:?}"
        );
    }
}

#[cfg(test)]
mod local_initialiser_tests {
    use super::{ConstantIndex, CrateIndex, DispatchWalk};
    use std::collections::{BTreeMap, BTreeSet};

    /// Enumerate one function body, the way `enumerate` does, and return the
    /// route ids it produced.
    fn route_ids(block: syn::Block) -> Vec<String> {
        let constants = ConstantIndex {
            facts: BTreeMap::new(),
            crates: BTreeSet::new(),
        };
        let index = CrateIndex::default();
        let mut walk = DispatchWalk {
            label: "core",
            index: &index,
            predicates: &index,
            constants: &constants,
            routes: Vec::new(),
            unclassified: Vec::new(),
            visited: BTreeSet::new(),
            declined_entries: 0,
        };
        walk.walk_block(
            &block,
            "src/resolution.rs",
            "resolution::process",
            0,
            None,
            &[],
            &[],
        );
        let mut ids: Vec<String> = walk.routes.into_iter().map(|route| route.id).collect();
        ids.sort();
        ids
    }

    /// THE DEFECT, and the reason this module exists.
    ///
    /// `f6b84c56` welded Core's `CloseFund` shut and had to write its guard as a
    /// statement-position `match` writing a deferred binding, because the
    /// obvious `let action = match …` DELETED
    /// `core/resolution::process#CloseFund` from the register while the refusal
    /// stayed live on chain. The two forms decide the same thing and must
    /// enumerate the same way.
    #[test]
    fn the_expression_form_of_a_guard_names_the_same_routes_as_the_statement_form() {
        let statement_form = route_ids(syn::parse_quote! {{
            let action;
            match resolution_request.action {
                ResolutionCoreActionV1::CloseFund => {
                    return Err(CoreSbfError::UnsupportedAction.into());
                }
                ResolutionCoreActionV1::CreateFund => action = Composed::CreateFund,
                ResolutionCoreActionV1::VerifyFundReady => action = Composed::VerifyFundReady,
                ResolutionCoreActionV1::AdmitTerminal => action = Composed::AdmitTerminal,
            }
            Ok(())
        }});
        let expression_form = route_ids(syn::parse_quote! {{
            let action = match resolution_request.action {
                ResolutionCoreActionV1::CloseFund => {
                    return Err(CoreSbfError::UnsupportedAction.into());
                }
                ResolutionCoreActionV1::CreateFund => Composed::CreateFund,
                ResolutionCoreActionV1::VerifyFundReady => Composed::VerifyFundReady,
                ResolutionCoreActionV1::AdmitTerminal => Composed::AdmitTerminal,
            };
            Ok(())
        }});
        assert_eq!(statement_form, expression_form);
        assert!(
            statement_form.contains(&"core/resolution::process#CloseFund".to_string()),
            "the refused action must keep its row in BOTH forms: {statement_form:?}"
        );
    }

    /// THE NEGATIVE CONTROL, in the shape the tree actually shipped.
    ///
    /// Before this change the same body enumerated three routes instead of
    /// four, and the missing one was the refusal. Whole-tree, that was 159
    /// against 160.
    #[test]
    fn a_let_bound_guard_no_longer_loses_the_row_that_records_its_refusal() {
        let ids = route_ids(syn::parse_quote! {{
            let action = match resolution_request.action {
                ResolutionCoreActionV1::CloseFund => {
                    return Err(CoreSbfError::UnsupportedAction.into());
                }
                ResolutionCoreActionV1::CreateFund => Composed::CreateFund,
                ResolutionCoreActionV1::VerifyFundReady => Composed::VerifyFundReady,
                ResolutionCoreActionV1::AdmitTerminal => Composed::AdmitTerminal,
            };
            Ok(())
        }});
        assert_eq!(
            ids,
            vec![
                "core/resolution::process#AdmitTerminal".to_string(),
                "core/resolution::process#CloseFund".to_string(),
                "core/resolution::process#CreateFund".to_string(),
                "core/resolution::process#VerifyFundReady".to_string(),
            ]
        );
    }

    /// AND THE OPPOSITE CONTROL, which is what stops the fix from being worse
    /// than the defect.
    ///
    /// Descending into every initialiser took the tree from 160 routes to 246
    /// and put seventeen previously-classified positions into `unclassified`.
    /// A table indexed by an action is not a route: no branch of it decides
    /// whether the instruction may proceed, and none leaves the function.
    #[test]
    fn a_lookup_table_keyed_by_the_same_discriminant_is_not_a_route() {
        let ids = route_ids(syn::parse_quote! {{
            let expected_writable = match action {
                Composed::CreateFund => [true, false],
                Composed::VerifyFundReady => [false, false],
                Composed::AdmitTerminal => [false, false],
            };
            Ok(())
        }});
        assert!(
            ids.is_empty(),
            "a value table must contribute no routes: {ids:?}"
        );
    }

    /// One arm returning is enough, and it is enough because that arm is the
    /// decision: the rest of the match is what happens when it does not fire.
    #[test]
    fn one_diverging_arm_makes_the_whole_initialiser_a_dispatch() {
        let ids = route_ids(syn::parse_quote! {{
            let width = match header.action {
                Action::Narrow => NARROW_BYTES,
                Action::Wide => WIDE_BYTES,
                Action::Unsupported => return Err(Error::Instruction),
            };
            Ok(())
        }});
        assert_eq!(ids.len(), 3, "{ids:?}");
        assert!(ids.contains(&"core/resolution::process#Unsupported".to_string()));
    }

    /// A `let` bound to an `if` whose else-branch refuses is the same shape
    /// written with two branches instead of four arms.
    #[test]
    fn an_if_initialiser_that_refuses_in_one_branch_is_a_dispatch() {
        let ids = route_ids(syn::parse_quote! {{
            let bytes = if instruction_data.len() == SETTLE_BYTES {
                instruction_data
            } else {
                return Err(Error::Instruction);
            };
            Ok(())
        }});
        assert!(!ids.is_empty(), "a refusing if-initialiser is a dispatch");
    }
}

#[cfg(test)]
mod doc_tests {
    use super::doc_text;
    use syn::parse_quote;

    fn split(item: syn::Variant) -> (Option<String>, Option<String>) {
        doc_text(&item.attrs)
    }

    #[test]
    fn a_one_paragraph_comment_is_all_summary_and_no_detail() {
        let (summary, detail) = split(parse_quote! {
            /// Account count, order, privilege, or aliasing was invalid.
            AccountFrame = 0x1001
        });
        assert_eq!(
            summary.as_deref(),
            Some("Account count, order, privilege, or aliasing was invalid.")
        );
        assert_eq!(detail, None);
    }

    #[test]
    fn rationale_after_the_blank_line_never_reaches_the_summary() {
        let (summary, detail) = split(parse_quote! {
            /// The release's pinned deployment slot moved: the substrate was upgraded.
            ///
            /// Decision 0012. Not a corrupted account and not an attack: the exact
            /// upgrade authority the release names shipped new bytes.
            ReleaseSuperseded = 0x100D
        });
        assert_eq!(
            summary.as_deref(),
            Some("The release's pinned deployment slot moved: the substrate was upgraded.")
        );
        let detail = detail.expect("the rationale paragraph is carried, not dropped");
        assert!(detail.starts_with("Decision 0012."));
        assert!(!detail.contains("pinned deployment slot moved"));
    }

    #[test]
    fn a_summary_that_wraps_two_lines_is_one_sentence() {
        let (summary, detail) = split(parse_quote! {
            /// A moved role's consenting upgrade authority did not sign,
            /// or cannot.
            ReleaseLineageAuthorityMissing = 0x1011
        });
        assert_eq!(
            summary.as_deref(),
            Some("A moved role's consenting upgrade authority did not sign, or cannot.")
        );
        assert_eq!(detail, None);
    }

    #[test]
    fn several_rationale_paragraphs_stay_separate() {
        let (_, detail) = split(parse_quote! {
            /// Summary.
            ///
            /// First.
            ///
            /// Second.
            Variant = 1
        });
        assert_eq!(detail.as_deref(), Some("First.\n\nSecond."));
    }

    #[test]
    fn a_variant_with_no_doc_comment_has_neither() {
        let (summary, detail) = split(parse_quote! { Undocumented = 2 });
        assert_eq!(summary, None);
        assert_eq!(detail, None);
    }
}

#[cfg(test)]
mod resolution_tests {
    use super::index_source;

    /// A bare call names the FREE function, never the inherent method that
    /// shares its name.
    ///
    /// The adapter is routinely named for the transition it drives --
    /// Custody's `open_hoard` handler calls `ProjectedCustodyStateV2::
    /// open_hoard` -- so indexing methods without this rule made the pair
    /// ambiguous and the CAUTIOUS resolver refused both. That is the worst
    /// shape a refusal can take: five Custody route handlers stopped
    /// resolving at all and the route count did not move, because a route
    /// with no gate and a route whose handler could not be found print the
    /// same cell.
    #[test]
    fn a_bare_call_resolves_to_the_free_function_and_a_typed_one_to_the_method() {
        let index = index_source(
            "projected",
            "fn open_hoard() {}
             struct State {}
             impl State { fn open_hoard(&self) {} }",
        );
        let bare = index.resolve("open_hoard").expect("the free function");
        assert!(bare.self_type.is_none());
        let method = index
            .resolve_from("projected", "State::open_hoard")
            .expect("the method");
        assert_eq!(method.self_type.as_deref(), Some("State"));
        assert!(index.resolve_method("State", "open_hoard").is_some());
        assert!(index.resolve_method("Other", "open_hoard").is_none());
    }

    /// A method name carried by two types is refused, not guessed.
    #[test]
    fn a_method_name_on_two_types_has_no_sole_owner() {
        let index = index_source(
            "m",
            "struct A {}
             impl A { fn validate(&self) {} }
             struct B {}
             impl B { fn validate(&self) {} }",
        );
        assert!(index.sole_method("validate").is_none());
        assert!(index.resolve("validate").is_none());
        assert!(index.resolve_method("A", "validate").is_some());
    }

    /// A trait impl contributes no method: one name over many types is a guess.
    #[test]
    fn a_trait_impl_contributes_no_method() {
        let index = index_source(
            "m",
            "struct A {}
             impl Decode for A { fn decode(&self) {} }",
        );
        assert!(index.resolve_method("A", "decode").is_none());
        assert!(index.sole_method("decode").is_none());
    }

    /// A `Result<T>` return reads as `T`, which is what a `?` binds.
    #[test]
    fn a_return_type_is_unwrapped_through_one_generic_layer() {
        let index = index_source(
            "m",
            "fn read_state() -> Result<StateV2, ProgramError> { todo!() }
             struct StateV2 { inner: Inner }
             struct Inner {}",
        );
        assert_eq!(
            index
                .resolve("read_state")
                .expect("indexed")
                .output
                .as_deref(),
            Some("StateV2")
        );
        assert_eq!(index.field_type("StateV2", "inner"), Some("Inner"));
        assert_eq!(index.field_type("StateV2", "absent"), None);
    }

    #[test]
    fn one_public_reexport_resolves_to_its_declared_function() {
        let index = index_source(
            "",
            "mod wire { pub fn selects() {} }
             mod adapter { pub use dclutch_codec::wire::selects; }",
        );
        let resolved = index
            .resolve("adapter::selects")
            .expect("one public re-export");
        assert_eq!(resolved.module, "wire");
        assert_eq!(
            index.canonical_function_path("adapter::selects"),
            "dclutch_codec::wire::selects"
        );
    }

    /// Re-export descent is one hop. A second alias could be cyclic or depend
    /// on package rules this source index does not model, so it is unresolved.
    #[test]
    fn a_reexport_chain_is_not_guessed_through() {
        let index = index_source(
            "",
            "mod wire { pub fn selects() {} }
             mod middle { pub use dclutch_codec::wire::selects; }
             mod adapter { pub use crate::middle::selects; }",
        );
        assert!(index.resolve("adapter::selects").is_none());
        assert_eq!(
            index.canonical_function_path("adapter::selects"),
            "adapter::selects"
        );
    }
}

/// The census's own blind spot, made enumerable.
///
/// Every test here is about ONE question: what happens when a program selects a
/// route with a guard shape this walk has never been taught? Until 2026-09-07
/// the answer was "nothing" -- `push_route` declined the forward, the route
/// count did not move, and the register looked complete. Two live Custody
/// wires sat outside it for as long as they existed, and the only instrument
/// that ever noticed was a HUMAN planting a blocking entry for a route id they
/// had guessed and watching the report call it stale.
///
/// So the shape rule is now the guard's CONTENT, not its name, and a guard
/// whose content this walk cannot read is REPORTED rather than dropped. These
/// tests pin both halves, and the third one is the one that matters: it feeds
/// the walk a shape deliberately outside every rule it knows and requires it to
/// say so.
#[cfg(test)]
mod dispatch_shape_tests {
    use super::{ConstantIndex, DispatchWalk, Selector, index_source};
    use std::collections::{BTreeMap, BTreeSet};

    struct Walked {
        routes: Vec<String>,
        selectors: Vec<String>,
        reported: Vec<String>,
    }

    /// Walk one entry function, with `helpers` as the resolvable first-party
    /// namespace a guard call can be read out of.
    fn walk(entry: &str, helpers: &str) -> Walked {
        let constants = ConstantIndex {
            facts: BTreeMap::new(),
            crates: BTreeSet::new(),
        };
        let index = index_source("", entry);
        let helper_index = index_source("selecting", helpers);
        let mut walk = DispatchWalk {
            label: "example",
            index: &index,
            predicates: &helper_index,
            constants: &constants,
            routes: Vec::new(),
            unclassified: Vec::new(),
            visited: BTreeSet::new(),
            declined_entries: 0,
        };
        let entry_function = index
            .resolve("process_instruction")
            .expect("the entry function is indexed")
            .clone();
        walk.walk_function(&entry_function, 0, None, &[], &[]);
        Walked {
            routes: walk.routes.iter().map(|route| route.id.clone()).collect(),
            selectors: walk
                .routes
                .iter()
                .flat_map(|route| &route.selectors)
                .map(Selector::render)
                .collect(),
            reported: walk
                .unclassified
                .iter()
                .map(|entry| entry.reason.clone())
                .collect(),
        }
    }

    /// Custody's shape, verbatim in structure: a recogniser called `selects`.
    const SELECTS_ENTRY: &str = r#"
        pub fn process_instruction(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
            if upkeep_vault_v1::selects(instruction_data) {
                return upkeep_vault_v1::process(program_id, accounts, instruction_data);
            }
            Err(Error::Instruction.into())
        }
    "#;

    const SELECTS_HELPER: &str = r#"
        pub fn selects(instruction_data: &[u8]) -> bool {
            instruction_data.len() == UPKEEP_VAULT_REQUEST_BYTES_V1
                && instruction_data.get(..UPKEEP_VAULT_REQUEST_MAGIC_V1.len())
                    == Some(UPKEEP_VAULT_REQUEST_MAGIC_V1.as_slice())
        }
    "#;

    /// THE DEFECT this module exists for.
    ///
    /// `upkeep_vault_v1::selects` and `protocol_parameters_v1::selects` state
    /// an exact width and an eight-byte magic and forward to a named handler,
    /// and the census enumerated neither, because the recogniser rule was the
    /// call's NAME. Both routes are live wires of a deployed program.
    #[test]
    fn a_recogniser_the_census_can_read_names_its_route_whatever_it_is_called() {
        let walked = walk(SELECTS_ENTRY, SELECTS_HELPER);
        assert_eq!(walked.routes, vec!["example/upkeep_vault_v1::process"]);
        assert!(
            walked.reported.is_empty(),
            "a shape the walk can read is not a blind spot: {:?}",
            walked.reported
        );
        assert!(
            walked
                .selectors
                .iter()
                .any(|rendered| rendered.contains("UPKEEP_VAULT_REQUEST_MAGIC_V1")),
            "the route carries the magic its recogniser states: {:?}",
            walked.selectors
        );
        assert!(
            walked
                .selectors
                .iter()
                .any(|rendered| rendered.contains("UPKEEP_VAULT_REQUEST_BYTES_V1")),
            "the route carries the width its recogniser states: {:?}",
            walked.selectors
        );
    }

    /// THE SELF-TEST: a shape outside every rule the walk knows.
    ///
    /// `dispatch_table_hit` is a guard call whose body this walk cannot read --
    /// it is not called `is_`-anything and it states no magic and no width, so
    /// neither the name rule nor the content rule fires. The route behind it is
    /// therefore NOT enumerated, which is exactly the condition that has to be
    /// impossible to hold quietly. If a future dispatch shape lands and this
    /// test still passes while the tree's own census reports zero unclassified
    /// positions, the census has gone blind again.
    #[test]
    fn an_entry_guard_this_census_cannot_read_is_reported_instead_of_dropped() {
        let walked = walk(
            r#"
                pub fn process_instruction(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
                    if dispatch_table_hit(instruction_data) {
                        return novel_family_v1::process(program_id, accounts, instruction_data);
                    }
                    Err(Error::Instruction.into())
                }
            "#,
            r#"
                pub fn dispatch_table_hit(instruction_data: &[u8]) -> bool {
                    TABLE.iter().any(|row| row.matches(instruction_data))
                }
            "#,
        );
        assert!(
            walked.routes.is_empty(),
            "the shape is genuinely unreadable, so no route can be claimed: {:?}",
            walked.routes
        );
        assert_eq!(
            walked.reported.len(),
            1,
            "one unreadable entry guard, one report: {:?}",
            walked.reported
        );
        assert!(
            walked.reported[0].contains("states no wire discriminant"),
            "the report says what is wrong and what to do: {}",
            walked.reported[0]
        );
    }

    /// The other half of the ratchet: it must not cry wolf.
    ///
    /// A guard that REFUSES is not a dispatch decision, and an unguarded call
    /// is the handler's continuation rather than a public route. Both decline a
    /// route today and both are correct to; neither is a blind spot, and a
    /// detector that reported them would be turned off within a week.
    #[test]
    fn a_refusing_guard_and_an_unguarded_continuation_are_not_blind_spots() {
        let walked = walk(
            r#"
                pub fn process_instruction(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
                    if accounts.len() < REQUIRED_ACCOUNTS {
                        return Err(Error::AccountFrame.into());
                    }
                    if !accounts[0].is_writable {
                        return Err(Error::AccountFrame.into());
                    }
                    process_remaining_instruction(program_id, accounts, instruction_data)
                }
            "#,
            "",
        );
        assert!(
            walked.reported.is_empty(),
            "neither shape hides a route, so neither is reported: {:?}",
            walked.reported
        );
    }
}
