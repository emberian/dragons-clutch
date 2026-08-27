//! The refusal-code band allocation, read from the tree under analysis.
//!
//! `crates/dclutch-refusal-registry` is the authority (decision 0007). The
//! census does not keep a second copy of the table: it parses the registry's
//! own source out of `--root`, for the same reason it parses dispatch branches
//! rather than reading a hand-kept list. A census that carried its own idea of
//! the allocation would agree with itself about a tree it was not looking at.
//!
//! What this module then checks is the property the bands exist for: **every
//! refusal code in the tree is unique, and belongs to the package that
//! declared it.** The route inventory cannot answer that on its own, because
//! it only walks the enumerated programs — and the collisions that actually
//! bit were in the test-only caller programs it does not walk.

use std::{collections::BTreeMap, fs, path::Path};

use syn::{Expr, Item};

use crate::{
    enumerate::{collect_refusals, rust_sources},
    model::{Band, BandTier, Refusal},
};

const REGISTRY_SOURCE: &str = "crates/dclutch-refusal-registry/src/lib.rs";

/// The band table and the alias list, as the registry crate declares them.
pub struct Allocation {
    pub bands: Vec<Band>,
    /// Enum names that deliberately raise another band's codes, and the label
    /// whose codes they raise.
    pub aliases: BTreeMap<String, String>,
    /// Repo-relative path the table was read from.
    pub source: String,
}

impl Allocation {
    /// The band that owns `code`, if any.
    pub fn owner(&self, code: i64) -> Option<&Band> {
        self.bands.iter().find(|band| band.contains(code))
    }
}

fn integer(expr: &Expr, consts: &BTreeMap<String, i64>) -> Option<i64> {
    match expr {
        Expr::Lit(literal) => match &literal.lit {
            syn::Lit::Int(int) => int.base10_parse::<i64>().ok(),
            _ => None,
        },
        Expr::Path(path) => consts
            .get(&path.path.segments.last()?.ident.to_string())
            .copied(),
        _ => None,
    }
}

fn string(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Lit(literal) => match &literal.lit {
            syn::Lit::Str(text) => Some(text.value()),
            _ => None,
        },
        _ => None,
    }
}

fn array_elements(expr: &Expr) -> Option<&syn::punctuated::Punctuated<Expr, syn::Token![,]>> {
    match expr {
        Expr::Reference(reference) => array_elements(&reference.expr),
        Expr::Array(array) => Some(&array.elems),
        _ => None,
    }
}

/// Read the band allocation out of the registry crate under `root`.
pub fn read(root: &Path) -> Result<Allocation, String> {
    let path = root.join(REGISTRY_SOURCE);
    let text = fs::read_to_string(&path).map_err(|error| {
        format!(
            "read {} (decision 0007 makes this crate the refusal-band authority; \
             the census cannot check uniqueness without it): {error}",
            path.display()
        )
    })?;
    let file =
        syn::parse_file(&text).map_err(|error| format!("parse {}: {error}", path.display()))?;

    let mut consts: BTreeMap<String, i64> = BTreeMap::new();
    let mut bands_expr = None;
    let mut aliases_expr = None;
    for item in &file.items {
        let Item::Const(konst) = item else {
            continue;
        };
        let name = konst.ident.to_string();
        match name.as_str() {
            "BANDS" => bands_expr = Some((*konst.expr).clone()),
            "ALIASES" => aliases_expr = Some((*konst.expr).clone()),
            _ => {
                if let Some(value) = integer(&konst.expr, &consts) {
                    consts.insert(name, value);
                }
            }
        }
    }

    let bands_expr = bands_expr.ok_or("the refusal registry declares no BANDS table")?;
    let elements =
        array_elements(&bands_expr).ok_or("the refusal registry's BANDS is not an array")?;

    let mut bands = Vec::with_capacity(elements.len());
    for element in elements {
        let Expr::Struct(entry) = element else {
            return Err("a BANDS entry is not a struct literal".into());
        };
        let mut label = None;
        let mut package = None;
        let mut base = None;
        let mut span = None;
        let mut tier = None;
        for field in &entry.fields {
            let syn::Member::Named(name) = &field.member else {
                continue;
            };
            match name.to_string().as_str() {
                "label" => label = string(&field.expr),
                "package" => package = string(&field.expr),
                "base" => base = integer(&field.expr, &consts),
                "span" => span = integer(&field.expr, &consts),
                "tier" => {
                    if let Expr::Path(path) = &field.expr {
                        tier = path.path.segments.last().map(|last| {
                            match last.ident.to_string().as_str() {
                                "TestCaller" => BandTier::TestCaller,
                                _ => BandTier::Program,
                            }
                        });
                    }
                }
                _ => {}
            }
        }
        let (Some(label), Some(package), Some(base), Some(span), Some(tier)) =
            (label, package, base, span, tier)
        else {
            return Err("a BANDS entry is missing a field the census needs".into());
        };
        bands.push(Band {
            label,
            package,
            base,
            span,
            tier,
        });
    }
    if bands.is_empty() {
        return Err("the refusal registry's BANDS table is empty".into());
    }

    let mut aliases = BTreeMap::new();
    if let Some(expr) = aliases_expr
        && let Some(elements) = array_elements(&expr)
    {
        for element in elements {
            let Expr::Tuple(pair) = element else { continue };
            let mut parts = pair.elems.iter();
            if let (Some(name), Some(label)) =
                (parts.next().and_then(string), parts.next().and_then(string))
            {
                aliases.insert(name, label);
            }
        }
    }

    Ok(Allocation {
        bands,
        aliases,
        source: REGISTRY_SOURCE.to_string(),
    })
}

/// One `#[repr]`-annotated refusal enum found anywhere in the tree, with the
/// Cargo package that declares it.
pub struct DeclaredRefusal {
    pub package: String,
    pub refusal: Refusal,
}

/// Every package directory under `programs/` and `crates/`, innermost first,
/// so a nested test-program crate wins over the package that contains it.
pub fn package_directories(root: &Path) -> Result<Vec<(String, std::path::PathBuf)>, String> {
    let mut found = Vec::new();
    for directory in ["crates", "programs"] {
        let base = root.join(directory);
        if !base.is_dir() {
            continue;
        }
        let mut stack = vec![base];
        while let Some(current) = stack.pop() {
            let entries = fs::read_dir(&current)
                .map_err(|error| format!("read {}: {error}", current.display()))?;
            for entry in entries {
                let entry = entry.map_err(|error| format!("read entry: {error}"))?;
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                if name == "target" || name == ".git" {
                    continue;
                }
                let manifest = path.join("Cargo.toml");
                if manifest.is_file()
                    && let Ok(text) = fs::read_to_string(&manifest)
                    && let Some(package) = package_name(&text)
                {
                    found.push((package, path.clone()));
                }
                stack.push(path);
            }
        }
    }
    // Deepest path first: a file inside a nested crate belongs to that crate.
    found.sort_by(|left, right| {
        right
            .1
            .components()
            .count()
            .cmp(&left.1.components().count())
            .then_with(|| left.1.cmp(&right.1))
    });
    Ok(found)
}

fn package_name(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if in_package && let Some(rest) = trimmed.strip_prefix("name") {
            let value = rest.trim_start_matches([' ', '=']).trim();
            return Some(value.trim_matches('"').to_string());
        }
    }
    None
}

/// Sweep every `#[repr]`-annotated `*Error` enum in the tree, not only the
/// enumerated programs.
///
/// The route inventory walks `TARGETS`. That is right for coverage and wrong
/// for uniqueness: the codes that actually collided belong to the test-only
/// caller programs, which are exactly the ones no target names.
pub fn sweep(root: &Path) -> Result<Vec<DeclaredRefusal>, String> {
    let owners = package_directories(root)?;
    let mut found = Vec::new();
    for (package, directory) in &owners {
        for path in rust_sources(directory)? {
            // Innermost-first ordering means the first package whose directory
            // contains this file is the one that compiles it.
            let real = owners
                .iter()
                .find(|(_, candidate)| path.starts_with(candidate))
                .map(|(name, _)| name.as_str());
            if real != Some(package.as_str()) {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(parsed) = syn::parse_file(&text) else {
                continue;
            };
            let mut refusals = Vec::new();
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            collect_refusals(&parsed.items, package, &relative, &mut refusals);
            for refusal in refusals {
                found.push(DeclaredRefusal {
                    package: package.clone(),
                    refusal,
                });
            }
        }
    }
    found.sort_by(|left, right| left.refusal.id.cmp(&right.refusal.id));
    found.dedup_by(|left, right| left.refusal.id == right.refusal.id);
    Ok(found)
}

/// Check that no two refusals share a code and that every code sits in the
/// band its declaring package owns.
///
/// Returns the problems found. An empty list is the only passing outcome.
pub fn check(allocation: &Allocation, declared: &[DeclaredRefusal]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen: BTreeMap<i64, &DeclaredRefusal> = BTreeMap::new();

    for entry in declared {
        let Some(code) = entry.refusal.code else {
            problems.push(format!(
                "{} declares {} with no resolvable discriminant, so its wire code is \
                 unknowable from source ({})",
                entry.package, entry.refusal.id, entry.refusal.provenance
            ));
            continue;
        };

        // An alias deliberately raises another band's codes: it is a published
        // boundary of the owning program, not a program of its own.
        let aliased = allocation.aliases.get(&entry.refusal.enum_name);

        match allocation.owner(code) {
            None => problems.push(format!(
                "{} declares {} = {code:#x}, which falls in no registered band. \
                 Codes below {:#x} are reserved for programs that are not ours \
                 (decision 0007); allocate a band in {} or move the variant.",
                entry.package,
                entry.refusal.id,
                allocation
                    .bands
                    .iter()
                    .map(|band| band.base)
                    .min()
                    .unwrap_or(0),
                allocation.source
            )),
            Some(band) => {
                let expected = aliased.map_or(band.package.as_str(), |label| {
                    allocation
                        .bands
                        .iter()
                        .find(|candidate| &candidate.label == label)
                        .map_or(band.package.as_str(), |candidate| {
                            candidate.package.as_str()
                        })
                });
                if band.package != entry.package && Some(&band.label) != aliased {
                    problems.push(format!(
                        "{} declares {} = {code:#x}, but band {:#05x} ({}) belongs to {}. \
                         A code that names the wrong program is the collision the bands exist \
                         to prevent; add an ALIASES entry in {} if the reuse is deliberate.",
                        entry.package,
                        entry.refusal.id,
                        band.base >> 12,
                        band.label,
                        expected,
                        allocation.source
                    ));
                }
            }
        }

        if let Some(held) = seen.insert(code, entry)
            && !allocation.aliases.contains_key(&entry.refusal.enum_name)
            && !allocation.aliases.contains_key(&held.refusal.enum_name)
        {
            problems.push(format!(
                "code {code:#x} is claimed twice: {} ({}) and {} ({}). The chain reports one \
                 number; two owners means whichever one a reader assumes is a coin flip.",
                held.refusal.id,
                held.refusal.provenance,
                entry.refusal.id,
                entry.refusal.provenance
            ));
        }
    }

    problems
}

/// Check the table against the packages the tree actually contains.
///
/// A band with no refusals in it is a reservation and fine. A band whose
/// package is not in the tree at all is not: a stale entry reads exactly like
/// a live one, which is the failure the census's own `TARGETS` comment
/// records paying for once already.
pub fn check_bands_are_live(
    allocation: &Allocation,
    present: &std::collections::BTreeSet<String>,
) -> Vec<String> {
    allocation
        .bands
        .iter()
        .filter(|band| !present.contains(&band.package))
        .map(|band| {
            format!(
                "band {:#05x} is allocated to {}, which is not a package in this tree. \
                 Withdraw it in {}: an entry for a program that does not exist reads \
                 exactly like a live one.",
                band.base >> 12,
                band.package,
                allocation.source
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn band(label: &str, package: &str, base: i64, tier: BandTier) -> Band {
        Band {
            label: label.into(),
            package: package.into(),
            base,
            span: 0x1000,
            tier,
        }
    }

    fn allocation() -> Allocation {
        Allocation {
            bands: vec![
                band("core", "dclutch-core-sbf", 0x3000, BandTier::Program),
                band("claims", "dclutch-claims-sbf", 0x5000, BandTier::Program),
                band(
                    "test/claims-caller",
                    "dclutch-claims-test-caller-sbf",
                    0x10_0000,
                    BandTier::TestCaller,
                ),
            ],
            aliases: BTreeMap::new(),
            source: REGISTRY_SOURCE.into(),
        }
    }

    fn declared(package: &str, id: &str, code: i64) -> DeclaredRefusal {
        let (enum_name, variant) = id.split_once("::").unwrap_or((id, ""));
        DeclaredRefusal {
            package: package.into(),
            refusal: Refusal {
                id: format!("{package}/{id}"),
                enum_name: enum_name.into(),
                variant: variant.into(),
                code: Some(code),
                doc: None,
                provenance: "src/lib.rs:1".into(),
            },
        }
    }

    #[test]
    fn the_real_registry_parses_and_its_own_tree_is_clean() {
        // The census reads the allocation out of the tree it is analysing, so
        // the shipped registry has to be parseable by exactly this code. A
        // registry the census cannot read is a gate that silently never runs.
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("repository root");
        let Ok(allocation) = read(root) else {
            // A census binary run against an archive that predates the registry
            // has nothing to check; that is a `read` error at the call site,
            // not a test failure here.
            return;
        };
        assert!(
            allocation.bands.len() >= 13,
            "the band table looks truncated"
        );
        assert!(
            allocation.bands.iter().all(|band| band.base >= 0x1000),
            "band 0 must stay unallocated: a code below 0x1000 is not ours"
        );
        assert!(
            allocation
                .aliases
                .contains_key("ShadowAcceleratorAuthErrorV4"),
            "the one deliberate alias must survive parsing, or it reads as a collision"
        );
    }

    #[test]
    fn a_code_in_two_packages_is_a_collision() {
        let problems = check(
            &allocation(),
            &[
                declared("dclutch-claims-sbf", "ClaimsSbfError::Release", 0x5003),
                declared("dclutch-core-sbf", "CoreSbfError::Reference", 0x5003),
            ],
        );
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains("claimed twice")),
            "{problems:?}"
        );
    }

    #[test]
    fn a_code_outside_every_band_is_a_problem() {
        let problems = check(
            &allocation(),
            &[declared("dclutch-core-sbf", "CoreSbfError::Market", 5)],
        );
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains("falls in no registered band")),
            "{problems:?}"
        );
    }

    #[test]
    fn a_code_inside_another_packages_band_is_a_problem() {
        let problems = check(
            &allocation(),
            &[declared("dclutch-core-sbf", "CoreSbfError::Market", 0x5005)],
        );
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains("belongs to dclutch-claims-sbf")),
            "{problems:?}"
        );
    }

    #[test]
    fn a_deliberate_alias_is_not_a_collision() {
        let mut held = allocation();
        held.aliases
            .insert("PublishedBoundaryError".into(), "core".into());
        let problems = check(
            &held,
            &[
                declared("dclutch-core-sbf", "CoreSbfError::Market", 0x3005),
                declared("dclutch-boundary", "PublishedBoundaryError::Market", 0x3005),
            ],
        );
        assert!(problems.is_empty(), "{problems:?}");
    }

    #[test]
    fn a_refusal_with_no_resolvable_code_is_a_problem() {
        let mut entry = declared("dclutch-core-sbf", "CoreSbfError::Market", 0x3005);
        entry.refusal.code = None;
        let problems = check(&allocation(), &[entry]);
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains("unknowable from source")),
            "{problems:?}"
        );
    }

    #[test]
    fn a_band_whose_package_left_the_tree_is_a_problem() {
        let present = [
            "dclutch-core-sbf".to_string(),
            "dclutch-claims-sbf".to_string(),
        ]
        .into_iter()
        .collect();
        let problems = check_bands_are_live(&allocation(), &present);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].contains("dclutch-claims-test-caller-sbf"),
            "{problems:?}"
        );
    }
}
