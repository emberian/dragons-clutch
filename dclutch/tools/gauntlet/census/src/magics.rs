//! Uniqueness of the protocol's 8-byte instruction and record magics.
//!
//! # Why this is a gate and not a report
//!
//! `AGENTS.md` names `inventory --check-unique` as *the* gate, and until now it
//! checked refusal bands only. A refusal code and an instruction magic are the
//! same kind of object -- a wire discriminant a program dispatches on -- and
//! only one of them had a uniqueness rule.
//!
//! The measured consequence, and the reason this exists:
//! `DEALER_SCENARIO_CHECKPOINT_RESERVE_MAGIC_V1`
//! (`programs/dclutch-trading-sbf/src/dealer_scenario_checkpoint_v1.rs`) and
//! `DIRECT_REPLAY_SETUP_REQUEST_MAGIC_V1`
//! (`crates/dclutch-direct-codec/src/replay_setup_v1.rs`) are both `DCLTDRS1`,
//! and both are TOP-LEVEL SELECTORS OF THE SAME TRADING ELF. Nothing separates
//! them but instruction length -- the dealer arm requires `data == MAGIC`,
//! exactly 8 bytes; the Direct arm requires exactly 120 -- and dispatch order,
//! `src/lib.rs:546` before `:601`. A mis-sized Direct replay-setup request does
//! not refuse: it routes into the Dealer family. Any future widening of the
//! bare dealer instruction collides silently, and the collision is a
//! wrong-handler bug, not a decode error.
//!
//! # What counts as a collision
//!
//! **One magic value claimed by two or more distinct constant names.** That is
//! the property that makes a wire byte ambiguous about which thing it selects.
//!
//! Re-declaring the SAME name in another package is a different defect -- a
//! mirror, in `AGENTS.md`'s sense -- and it is counted and printed separately
//! rather than failing this gate. That split is deliberate and is stated here
//! so nobody has to guess whether the gate was narrowed to whatever passed:
//! a mirror is one fact with two authors, which is a convergence problem; a
//! collision is two facts with one wire encoding, which is a dispatch-safety
//! problem. Only the second can route a caller into the wrong handler.
//!
//! A collision is NOT fixed by re-lettering one side to make this gate green.
//! Some of these are shipped instruction discriminants, where changing one is a
//! wire event needing its own decision record exactly as a refusal-band
//! renumbering does; others are record magics a fixture re-declared under a
//! local name, where the fix is to import the canonical constant. Which is
//! which is an adjudication, and the gate's job is to force it rather than to
//! guess it.

use std::{collections::BTreeMap, fs, path::Path};

use syn::{Expr, Item, UnOp};

use crate::enumerate::rust_sources;

/// One `const NAME: [u8; 8] = *b"........";` as the tree declares it.
#[derive(Clone, Debug)]
pub struct DeclaredMagic {
    pub package: String,
    /// The constant's own identifier.
    pub name: String,
    /// The eight bytes, rendered as ASCII when they are printable.
    pub value: String,
    /// `path:line`, repo-relative.
    pub provenance: String,
}

/// Render eight bytes the way a reader greps for them.
fn render(bytes: &[u8]) -> String {
    if bytes.iter().all(|byte| byte.is_ascii_graphic()) {
        String::from_utf8_lossy(bytes).into_owned()
    } else {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

/// The literal behind `*b"...."`, `b"...."`, or a plain byte-string const.
fn byte_string(expr: &Expr) -> Option<Vec<u8>> {
    match expr {
        // `*b"DCLTDRS1"` -- the idiom the tree uses.
        Expr::Unary(unary) if matches!(unary.op, UnOp::Deref(_)) => byte_string(&unary.expr),
        Expr::Lit(literal) => match &literal.lit {
            syn::Lit::ByteStr(bytes) => Some(bytes.value()),
            _ => None,
        },
        _ => None,
    }
}

/// Whether a type is exactly `[u8; 8]`.
fn is_eight_byte_array(ty: &syn::Type) -> bool {
    let syn::Type::Array(array) = ty else {
        return false;
    };
    let syn::Type::Path(path) = array.elem.as_ref() else {
        return false;
    };
    if !path.path.is_ident("u8") {
        return false;
    }
    let Expr::Lit(literal) = &array.len else {
        return false;
    };
    match &literal.lit {
        syn::Lit::Int(int) => int.base10_parse::<usize>().is_ok_and(|len| len == 8),
        _ => false,
    }
}

fn line_of(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())]
        .bytes()
        .filter(|b| *b == b'\n')
        .count()
        + 1
}

fn collect(
    items: &[Item],
    package: &str,
    relative: &str,
    source: &str,
    found: &mut Vec<DeclaredMagic>,
) {
    for item in items {
        match item {
            Item::Mod(module) => {
                if let Some((_, inner)) = &module.content {
                    collect(inner, package, relative, source, found);
                }
            }
            Item::Const(konst) if is_eight_byte_array(&konst.ty) => {
                let Some(bytes) = byte_string(&konst.expr) else {
                    continue;
                };
                if bytes.len() != 8 {
                    continue;
                }
                // `proc_macro2` spans carry no byte offset without the
                // `span-locations` feature, so the line is recovered by
                // finding the declaration in the source text. Exact enough for
                // a provenance string, and it never fabricates one.
                let needle = format!("{}", konst.ident);
                let line = source
                    .find(&format!("const {needle}"))
                    .map_or(0, |offset| line_of(source, offset));
                found.push(DeclaredMagic {
                    package: package.to_string(),
                    name: needle,
                    value: render(&bytes),
                    provenance: format!("{relative}:{line}"),
                });
            }
            _ => {}
        }
    }
}

/// Every 8-byte magic declared under `crates/` and `programs/`.
///
/// Scoped to the two protocol directories on purpose: `tools/` carries
/// deliberate operator-side copies of protocol constants, which are a
/// convergence question owned elsewhere, not a dispatch-safety one.
pub fn sweep(root: &Path) -> Result<Vec<DeclaredMagic>, String> {
    let owners = super::bands::package_directories(root)?;
    let mut found = Vec::new();
    for (package, directory) in &owners {
        for path in rust_sources(directory)? {
            // Innermost-first: the first package whose directory contains this
            // file is the one that compiles it.
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
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            collect(&parsed.items, package, &relative, &text, &mut found);
        }
    }
    found.sort_by(|left, right| {
        left.value
            .cmp(&right.value)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.provenance.cmp(&right.provenance))
    });
    found.dedup_by(|left, right| {
        left.value == right.value && left.name == right.name && left.provenance == right.provenance
    });
    Ok(found)
}

/// How many magics were declared, and how many are same-name mirrors.
pub struct MagicSummary {
    pub declared: usize,
    pub distinct: usize,
    /// Magic values re-declared under the same constant name in another
    /// package. Not a gate failure; see the module documentation.
    pub mirrored: Vec<String>,
}

/// One magic value claimed by two or more distinct constant names is a
/// collision. Returns the problems; an empty list is the only passing outcome.
pub fn check(declared: &[DeclaredMagic]) -> (Vec<String>, MagicSummary) {
    let mut by_value: BTreeMap<&str, Vec<&DeclaredMagic>> = BTreeMap::new();
    for magic in declared {
        by_value.entry(&magic.value).or_default().push(magic);
    }

    let mut problems = Vec::new();
    let mut mirrored = Vec::new();
    for (value, entries) in &by_value {
        let mut names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        if names.len() > 1 {
            let sites = entries
                .iter()
                .map(|entry| format!("{} ({})", entry.name, entry.provenance))
                .collect::<Vec<_>>()
                .join(", ");
            problems.push(format!(
                "magic `{value}` is claimed by {} different constants: {sites}. A wire \
                 discriminant selects one thing. Adjudicate it -- have one side import the \
                 canonical constant, or renumber under a decision record. Do NOT re-letter one \
                 side to make this gate green: these are shipped discriminants, and for the two \
                 that are live top-level selectors of the same ELF the collision is a \
                 wrong-handler question, not a naming one.",
                names.len()
            ));
        } else if entries.len() > 1 {
            let packages = entries
                .iter()
                .map(|entry| entry.package.as_str())
                .collect::<std::collections::BTreeSet<_>>();
            if packages.len() > 1 {
                mirrored.push(format!(
                    "{value} = {} in {} packages",
                    names[0],
                    packages.len()
                ));
            }
        }
    }

    let summary = MagicSummary {
        declared: declared.len(),
        distinct: by_value.len(),
        mirrored,
    };
    (problems, summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn magic(package: &str, name: &str, value: &str, provenance: &str) -> DeclaredMagic {
        DeclaredMagic {
            package: package.into(),
            name: name.into(),
            value: value.into(),
            provenance: provenance.into(),
        }
    }

    /// The measured case this gate exists for: two live top-level selectors of
    /// ONE ELF sharing a discriminant, separated only by instruction length.
    #[test]
    fn two_constants_sharing_one_magic_is_a_collision() {
        let declared = [
            magic(
                "dclutch-trading-sbf",
                "DEALER_SCENARIO_CHECKPOINT_RESERVE_MAGIC_V1",
                "DCLTDRS1",
                "programs/dclutch-trading-sbf/src/dealer_scenario_checkpoint_v1.rs:88",
            ),
            magic(
                "dclutch-direct-codec",
                "DIRECT_REPLAY_SETUP_REQUEST_MAGIC_V1",
                "DCLTDRS1",
                "crates/dclutch-direct-codec/src/replay_setup_v1.rs:13",
            ),
        ];
        let (problems, summary) = check(&declared);
        assert_eq!(problems.len(), 1, "one collision, reported once");
        assert!(problems[0].contains("DCLTDRS1"));
        assert!(problems[0].contains("DEALER_SCENARIO_CHECKPOINT_RESERVE_MAGIC_V1"));
        assert!(problems[0].contains("DIRECT_REPLAY_SETUP_REQUEST_MAGIC_V1"));
        assert!(
            summary.mirrored.is_empty(),
            "distinct names are a collision, never a mirror"
        );
    }

    /// A same-name re-declaration is a convergence defect, not a dispatch one.
    /// It is counted and printed, and it does NOT fail the gate. This test is
    /// the record of that decision: if someone later makes mirrors fail, they
    /// have to delete an assertion that says why they did not.
    #[test]
    fn a_same_name_mirror_is_counted_but_does_not_fail_the_gate() {
        let declared = [
            magic(
                "dclutch-trading-sbf",
                "PROJECTED_CUSTODY_ABORT_MAGIC_V1",
                "DCLTPCA1",
                "programs/dclutch-trading-sbf/src/projected_custody_bootstrap_v1.rs:1",
            ),
            magic(
                "dclutch-svm-harness",
                "PROJECTED_CUSTODY_ABORT_MAGIC_V1",
                "DCLTPCA1",
                "crates/dclutch-svm-harness/tests/controller_funding_split_abort.rs:1",
            ),
        ];
        let (problems, summary) = check(&declared);
        assert!(problems.is_empty(), "a mirror is not a gate failure");
        assert_eq!(summary.mirrored.len(), 1);
        assert!(summary.mirrored[0].contains("DCLTPCA1"));
    }

    /// One constant declared once is the ordinary case and must stay silent.
    #[test]
    fn a_unique_magic_raises_nothing() {
        let declared = [magic("p", "A_MAGIC_V1", "DCLTAAA1", "a.rs:1")];
        let (problems, summary) = check(&declared);
        assert!(problems.is_empty());
        assert!(summary.mirrored.is_empty());
        assert_eq!(summary.distinct, 1);
    }

    /// The parser must take `*b"..."` and must not take a differently-sized or
    /// differently-typed constant that happens to sit next to one.
    #[test]
    fn only_eight_byte_byte_string_constants_are_swept() {
        let source = r#"
            pub const GOOD_MAGIC_V1: [u8; 8] = *b"DCLTGOOD";
            pub const SHORT_MAGIC: [u8; 4] = *b"DCLT";
            pub const NOT_BYTES: [u8; 8] = [0; 8];
            pub const WIDTH: usize = 8;
            mod inner {
                pub const NESTED_MAGIC_V1: [u8; 8] = *b"DCLTNEST";
            }
        "#;
        let parsed = syn::parse_file(source).expect("parses");
        let mut found = Vec::new();
        collect(&parsed.items, "p", "a.rs", source, &mut found);
        let names: Vec<&str> = found.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["GOOD_MAGIC_V1", "NESTED_MAGIC_V1"]);
        assert_eq!(found[0].value, "DCLTGOOD");
        assert_eq!(found[1].value, "DCLTNEST", "a magic inside a module counts");
    }
}
