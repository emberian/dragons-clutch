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

use crate::model::{
    Entrypoint, Inventory, ProgramSurface, Provenance, Refusal, Route, RouteKind, Selector,
    Unclassified,
};

/// Minimal token rendering. `syn`'s `printing` feature gives us
/// `ToTokens`; we only ever need a one-line human-readable form.
mod quote_min {
    use quote::ToTokens;

    pub fn render<T: ToTokens>(value: &T) -> String {
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
    pub fn render_path<T: ToTokens>(value: &T) -> String {
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
}

/// Index every `const NAME: ... = <literal>;` in the tree that we can evaluate.
pub fn index_constants(root: &Path) -> Result<ConstantIndex, String> {
    let mut index = ConstantIndex::default();
    for directory in ["crates", "programs"] {
        let base = root.join(directory);
        if !base.is_dir() {
            continue;
        }
        for path in rust_sources(&base)? {
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(file) = syn::parse_file(&text) else {
                continue;
            };
            let relative = relative(root, &path);
            index_constants_in_items(&file.items, &relative, &mut index);
        }
    }
    Ok(index)
}

fn index_constants_in_items(items: &[Item], relative: &str, index: &mut ConstantIndex) {
    for item in items {
        match item {
            Item::Const(konst) => {
                if let Some(value) = constant_value(&konst.expr) {
                    index
                        .facts
                        .entry(konst.ident.to_string())
                        .or_default()
                        .push(ConstantFact {
                            value,
                            provenance: at(relative, konst.ident.span()),
                        });
                }
            }
            Item::Mod(module) => {
                if let Some((_, items)) = &module.content {
                    index_constants_in_items(items, relative, index);
                }
            }
            Item::Impl(block) => {
                for item in &block.items {
                    if let syn::ImplItem::Const(konst) = item
                        && let Some(value) = constant_value(&konst.expr)
                    {
                        index
                            .facts
                            .entry(konst.ident.to_string())
                            .or_default()
                            .push(ConstantFact {
                                value,
                                provenance: at(relative, konst.ident.span()),
                            });
                    }
                }
            }
            _ => {}
        }
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

fn rust_sources(base: &Path) -> Result<Vec<PathBuf>, String> {
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

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn at(relative: &str, span: Span) -> Provenance {
    format!("{relative}:{}", span.start().line)
}

/// One function the enumerator can follow into, keyed by module path.
struct FunctionFact {
    module: String,
    name: String,
    block: Block,
    relative: String,
}

/// A parsed program crate: every function in it, indexed for call resolution.
struct CrateIndex {
    functions: Vec<FunctionFact>,
}

impl CrateIndex {
    /// Resolve `a::b::name` (or bare `name`) to a function in this crate.
    /// A path whose module qualifier does not match anything is unresolved,
    /// which the caller reports rather than guessing at.
    fn resolve(&self, path: &str) -> Option<&FunctionFact> {
        let segments: Vec<&str> = path.split("::").collect();
        let name = *segments.last()?;
        let qualifier = if segments.len() >= 2 {
            Some(segments[segments.len() - 2])
        } else {
            None
        };
        let mut matches = self.functions.iter().filter(|fact| {
            fact.name == name
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

fn index_crate(root: &Path, crate_src: &Path) -> Result<CrateIndex, String> {
    let mut functions = Vec::new();
    for path in rust_sources(crate_src)? {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(file) = syn::parse_file(&text) else {
            continue;
        };
        let module = module_path_for(crate_src, &path);
        let relative = relative(root, &path);
        collect_functions(&file.items, &module, &relative, &mut functions);
    }
    Ok(CrateIndex { functions })
}

fn collect_functions(items: &[Item], module: &str, relative: &str, out: &mut Vec<FunctionFact>) {
    for item in items {
        match item {
            Item::Fn(function) => out.push(FunctionFact {
                module: module.to_string(),
                name: function.sig.ident.to_string(),
                block: (*function.block).clone(),
                relative: relative.to_string(),
            }),
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

fn collect_refusals(items: &[Item], label: &str, relative: &str, out: &mut Vec<Refusal>) {
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
        out.push(Refusal {
            id: format!("{label}/{enum_name}::{variant_name}"),
            enum_name: enum_name.clone(),
            variant: variant_name,
            code,
            doc: doc_text(&variant.attrs),
            provenance: at(relative, variant.ident.span()),
        });
    }
}

fn doc_text(attributes: &[Attribute]) -> Option<String> {
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
    if lines.is_empty() {
        None
    } else {
        Some(lines.join(" "))
    }
}

// ------------------------------------------------------------------ dispatch

struct DispatchWalk<'a> {
    label: &'a str,
    index: &'a CrateIndex,
    constants: &'a ConstantIndex,
    routes: Vec<Route>,
    unclassified: Vec<Unclassified>,
    visited: BTreeSet<String>,
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
                Stmt::Local(_) | Stmt::Item(_) | Stmt::Macro(_) => {}
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
                self.walk_block(
                    &branch.then_branch,
                    relative,
                    context,
                    depth,
                    parent,
                    &selectors,
                    &branch_cfg,
                );
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
                    let function = FunctionFact {
                        module: function.module.clone(),
                        name: function.name.clone(),
                        block: function.block.clone(),
                        relative: function.relative.clone(),
                    };
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
            Expr::Unsafe(_) => {
                self.report(context, expr, relative, "unsafe block in dispatch position");
            }
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
                        None => vec![Selector::Variant { path }],
                    }
                }
                None => vec![Selector::Tag { text: render(pat) }],
            },
        }
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
                    let target = render_path(&path.path);
                    let name = target.rsplit("::").next().unwrap_or(&target);
                    if name.starts_with("is_") {
                        out.push(Selector::Predicate {
                            function: target.clone(),
                        });
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
    targets: &[ProgramTarget],
    constants: &ConstantIndex,
    source_revision: Option<String>,
) -> Result<Inventory, String> {
    let mut programs = Vec::new();
    for target in targets {
        let crate_src = root.join("programs").join(&target.package).join("src");
        let lib = crate_src.join("lib.rs");
        if !lib.is_file() {
            return Err(format!("no crate root at {}", lib.display()));
        }
        let index = index_crate(root, &crate_src)?;
        let text =
            fs::read_to_string(&lib).map_err(|error| format!("read {}: {error}", lib.display()))?;
        let file: File =
            syn::parse_file(&text).map_err(|error| format!("parse {}: {error}", lib.display()))?;
        let lib_relative = relative(root, &lib);

        let mut refusals = Vec::new();
        for path in rust_sources(&crate_src)? {
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(parsed) = syn::parse_file(&text) else {
                continue;
            };
            collect_refusals(
                &parsed.items,
                &target.label,
                &relative(root, &path),
                &mut refusals,
            );
        }
        refusals.sort_by(|left, right| left.id.cmp(&right.id));
        refusals.dedup_by(|left, right| left.id == right.id);

        let mut walk = DispatchWalk {
            label: &target.label,
            index: &index,
            constants,
            routes: Vec::new(),
            unclassified: Vec::new(),
            visited: BTreeSet::new(),
        };

        let mut entrypoints = Vec::new();
        for (macro_name, function_name, provenance) in find_entrypoints(&file.items, &lib_relative)
        {
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
            let function = FunctionFact {
                module: function.module.clone(),
                name: function.name.clone(),
                block: function.block.clone(),
                relative: function.relative.clone(),
            };
            // The entrypoint shim usually forwards straight to
            // `process_instruction`. Walking it at depth 0 makes that forward a
            // route; unwrap it so the real dispatch branches are the entries.
            let forwarded = single_forward(&function.block);
            let dispatch = match forwarded.as_deref().and_then(|name| index.resolve(name)) {
                Some(inner) => FunctionFact {
                    module: inner.module.clone(),
                    name: inner.name.clone(),
                    block: inner.block.clone(),
                    relative: inner.relative.clone(),
                },
                None => function,
            };
            // The entrypoint itself is always a route. A program whose dispatch
            // has no internal branch structure still has exactly one public
            // entry, and the census must be able to say whether it has ever
            // executed rather than showing an empty table.
            walk.push_entry_route(&dispatch);
            walk.walk_function(&dispatch, 0, None, &[], &[]);
        }

        let mut routes = walk.routes;
        routes.sort_by(|left, right| left.id.cmp(&right.id));
        let mut unclassified = walk.unclassified;
        unclassified.sort_by(|left, right| left.provenance.cmp(&right.provenance));

        programs.push(ProgramSurface {
            package: target.package.clone(),
            label: target.label.clone(),
            crate_root: lib_relative,
            entrypoints,
            routes,
            refusals,
            unclassified,
        });
    }

    Ok(Inventory {
        schema: crate::model::INVENTORY_SCHEMA_V1.into(),
        source_root: root.to_string_lossy().into_owned(),
        source_revision,
        programs,
    })
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

/// A short, line-stable fingerprint of a route's most specific wire
/// discriminant. Route ids must survive an unrelated edit moving the source
/// line, so ids are built from selector names rather than positions.
fn specific_tag(selectors: &[Selector]) -> String {
    let mut last = None;
    let mut fallthrough = false;
    for selector in selectors {
        match selector {
            Selector::Variant { path } => {
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
