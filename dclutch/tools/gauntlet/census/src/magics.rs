//! Uniqueness of the protocol's 8-byte instruction and record magics.
//!
//! # Why this is a gate and not a report
//!
//! `AGENTS.md` names `inventory --check-unique` as *the* gate, and until now it
//! checked refusal bands only. A refusal code and an instruction magic are the
//! same kind of object -- a wire discriminant a program dispatches on -- and
//! only one of them had a uniqueness rule.
//!
//! The measured consequence, and the reason this exists (now FIXED, and the
//! gate is what keeps it fixed): `DEALER_SCENARIO_CHECKPOINT_RESERVE_MAGIC_V1`
//! (`programs/dclutch-trading-sbf/src/dealer_scenario_checkpoint_v1.rs`) and
//! `DIRECT_REPLAY_SETUP_REQUEST_MAGIC_V1`
//! (`crates/dclutch-trading/src/replay_setup_v1.rs`) were both `DCLTDRS1`,
//! and both are TOP-LEVEL SELECTORS OF THE SAME TRADING ELF. Nothing separated
//! them but instruction length -- the dealer arm requires `data == MAGIC`,
//! exactly 8 bytes; the Direct arm requires exactly 120 -- and dispatch order,
//! `src/lib.rs:546` before `:601`. A mis-sized Direct replay-setup request did
//! not refuse: it routed into the Dealer family. Any future widening of the
//! bare dealer instruction would have collided silently, and the collision is a
//! wrong-handler bug, not a decode error. The dealer side is now `DCLTDRV1`;
//! the tests below keep the original pair as the worked example, because it is
//! the case this rule exists to catch.
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

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::Deserialize;
use syn::{Expr, Item, UnOp};


/// Repo-relative path of the adjudicated-collision register.
pub const EXEMPTIONS_PATH: &str = "tools/gauntlet/magic-collisions.json";

/// Shortest verdict the gate will accept.
///
/// Not a style rule. An exemption that records nothing can be added by anyone
/// in a hurry; one that has to be argued cannot, and that difference is the
/// entire reason this register is a fact rather than a mute switch. The
/// threshold is deliberately low enough that a real verdict clears it without
/// thought and high enough that "n/a", "safe" or "" does not.
const MINIMUM_VERDICT: usize = 80;

#[derive(Debug, Deserialize)]
pub struct Exemptions {
    #[serde(default)]
    #[allow(dead_code, reason = "documentation for a human reading the file")]
    pub note: String,
    pub exempt: Vec<Exemption>,
}

/// One adjudicated collision: this exact set of names may share this value,
/// for this written reason.
#[derive(Debug, Deserialize)]
pub struct Exemption {
    pub magic: String,
    /// The EXACT set of constant names observed at the magic. A third claimant
    /// makes the set stop matching, so the collision fires again: an exemption
    /// pins a fact, it does not silence a value.
    pub constants: Vec<String>,
    /// Why THIS sharing cannot mis-dispatch. Required; see [`MINIMUM_VERDICT`].
    pub verdict: String,
    pub owner: String,
}

/// Read the register. A missing file is an empty register, not an error: the
/// gate's default posture is to fail on every collision.
pub fn read_exemptions(root: &Path) -> Result<Vec<Exemption>, String> {
    let path = root.join(EXEMPTIONS_PATH);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let bytes = fs::read(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let parsed: Exemptions = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse {}: {error}", path.display()))?;
    Ok(parsed.exempt)
}

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
    /// Whether the declaration is written as an export -- `pub` or `pub(...)`
    /// -- rather than a file-private `const`. Only an export can be named by
    /// someone else, which is what makes [`check_names`] a dispatch-safety
    /// question rather than a style one.
    pub exported: bool,
}

/// Render eight bytes the way a reader greps for them.
fn render(bytes: &[u8]) -> String {
    if bytes.iter().all(|byte| byte.is_ascii_graphic()) {
        String::from_utf8_lossy(bytes).into_owned()
    } else {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

/// The eight bytes behind a magic declaration, in either idiom the tree uses.
///
/// `*b"DCLTDRS1"` is what a HAND-WRITTEN magic looks like. A Lean-EMITTED one is
/// a hex array -- `[0x44, 0x43, 0x4c, 0x54, ...]` -- because an emitter prints
/// bytes and not text, and every magic this repository has moved to a Lean owner
/// takes that form.
///
/// Reading only the first idiom made this gate blind to 51 of the 278 declared
/// magics, and the hole GREW with every layout that gained a Lean owner: a gate
/// that goes blind in proportion to how much of the tree is properly authored is
/// pointed the wrong way. Hidden inside that blindness was a real collision.
///
/// One parser reads both, so a magic is counted by what it declares and not by
/// which backend happened to write it down.
fn magic_bytes(expr: &Expr) -> Option<Vec<u8>> {
    match expr {
        // `*b"DCLTDRS1"` -- the hand-written idiom.
        Expr::Unary(unary) if matches!(unary.op, UnOp::Deref(_)) => magic_bytes(&unary.expr),
        Expr::Group(group) => magic_bytes(&group.expr),
        Expr::Paren(paren) => magic_bytes(&paren.expr),
        Expr::Lit(literal) => match &literal.lit {
            syn::Lit::ByteStr(bytes) => Some(bytes.value()),
            _ => None,
        },
        // `[0x44, 0x43, ...]` -- the emitted idiom. Every element must be an
        // integer literal that fits in a `u8`; anything computed, negative or
        // referential is not a declaration this gate can read, and is skipped
        // rather than guessed at. `[0; 8]` is a `Repeat` and stays out: a zero
        // filler is a placeholder, not a wire discriminant.
        Expr::Array(array) => array
            .elems
            .iter()
            .map(|element| match element {
                Expr::Lit(literal) => match &literal.lit {
                    syn::Lit::Int(int) => int.base10_parse::<u8>().ok(),
                    _ => None,
                },
                _ => None,
            })
            .collect(),
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
                let Some(bytes) = magic_bytes(&konst.expr) else {
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
                    exported: !matches!(konst.vis, syn::Visibility::Inherited),
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
pub fn sweep(sources: &crate::sources::Sources) -> Vec<DeclaredMagic> {
    let mut found = Vec::new();
    for (package, directory) in &sources.packages {
        for source in sources.owned_by(package, directory) {
            collect(&source.file.items, package, &source.relative, &source.text, &mut found);
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
    found
}

/// How many magics were declared, and how many are same-name mirrors.
pub struct MagicSummary {
    pub declared: usize,
    pub distinct: usize,
    /// Magic values re-declared under the same constant name in another
    /// package. Not a gate failure; see the module documentation.
    pub mirrored: Vec<String>,
    /// Collisions excused by an argued verdict in the register.
    pub exempted: usize,
}

/// One magic value claimed by two or more distinct constant names is a
/// collision, unless `exemptions` carries an argued verdict for that exact set
/// of names. Returns the problems; an empty list is the only passing outcome.
pub fn check(declared: &[DeclaredMagic], exemptions: &[Exemption]) -> (Vec<String>, MagicSummary) {
    let mut by_value: BTreeMap<&str, Vec<&DeclaredMagic>> = BTreeMap::new();
    for magic in declared {
        by_value.entry(&magic.value).or_default().push(magic);
    }

    let mut problems = Vec::new();
    let mut mirrored = Vec::new();
    let mut exempted = 0_usize;
    let mut used: BTreeSet<&str> = BTreeSet::new();

    // A verdict is checked BEFORE it is allowed to excuse anything, so a
    // register entry cannot buy silence by existing.
    for exemption in exemptions {
        if exemption.verdict.trim().len() < MINIMUM_VERDICT {
            problems.push(format!(
                "magic exemption for `{}` in {EXEMPTIONS_PATH} carries no argued verdict \
                 ({} characters, {MINIMUM_VERDICT} required). An exemption that records nothing \
                 is a hidden failure; say which dispatcher can and cannot see both constants.",
                exemption.magic,
                exemption.verdict.trim().len()
            ));
        }
        if exemption.owner.trim().is_empty() {
            problems.push(format!(
                "magic exemption for `{}` in {EXEMPTIONS_PATH} names no owner",
                exemption.magic
            ));
        }
    }

    for (value, entries) in &by_value {
        let mut names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        if names.len() > 1 {
            // An exemption applies only to the EXACT set adjudicated. A new
            // claimant changes the set, and the collision fires again.
            let adjudicated = exemptions.iter().find(|exemption| {
                exemption.magic == *value && {
                    let mut listed: Vec<&str> =
                        exemption.constants.iter().map(String::as_str).collect();
                    listed.sort_unstable();
                    listed.dedup();
                    listed == names
                }
            });
            if let Some(exemption) = adjudicated
                && exemption.verdict.trim().len() >= MINIMUM_VERDICT
            {
                exempted += 1;
                used.insert(exemption.magic.as_str());
                continue;
            }
            if let Some(exemption) = exemptions
                .iter()
                .find(|exemption| exemption.magic == *value)
            {
                used.insert(exemption.magic.as_str());
                let mut listed: Vec<&str> =
                    exemption.constants.iter().map(String::as_str).collect();
                listed.sort_unstable();
                listed.dedup();
                if listed != names {
                    problems.push(format!(
                        "magic `{value}` is adjudicated in {EXEMPTIONS_PATH} for [{}], but the \
                         tree now declares it as [{}]. A new claimant is a NEW collision and has \
                         to be argued on its own terms, not inherited from the old verdict.",
                        listed.join(", "),
                        names.join(", ")
                    ));
                    continue;
                }
            }
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

    // A register entry for a collision that no longer exists is a false record.
    // Same rule as `tools/gauntlet/blocked.json`: keep an entry only while it is
    // true, and delete it the moment it stops being. This is what would have
    // caught `DCLTDRS1`'s entry surviving its own re-lettering.
    for exemption in exemptions {
        if !used.contains(exemption.magic.as_str()) {
            problems.push(format!(
                "magic `{}` is adjudicated in {EXEMPTIONS_PATH} but no longer collides in the \
                 tree. Delete the entry: a register of facts may not carry a stale one.",
                exemption.magic
            ));
        }
    }

    let summary = MagicSummary {
        declared: declared.len(),
        distinct: by_value.len(),
        mirrored,
        exempted,
    };
    (problems, summary)
}

/// One constant NAME bound to two or more different magics.
///
/// The exact inverse of [`check`], and the other half of the same
/// one-to-one rule: that gate asks whether a wire VALUE means one thing, this
/// one asks whether a NAME does. A value with two names can route a caller
/// into the wrong handler. A name with two values misleads the AUTHOR instead
/// -- whoever writes `use ...::REQUEST_MAGIC` gets whichever eight bytes their
/// crate happens to declare, and nothing tells them another crate spells
/// different bytes the same way.
///
/// The measured case this exists for: `EmitDealerLiquidityAbiRust.lean` and
/// `EmitGeneralControllerAbiRust.lean` both emitted a bare `REQUEST_MAGIC`, for
/// `DCDREQ01` and `DCGREQ01` -- and, with `EmitMarketCoreRust.lean`'s third
/// bare one for `DCLTCRQ2`, `ConstantIndex::resolve` refused the name as a
/// collision instead of answering it, so the route census could attribute none
/// of the three magics to any route. Core's was fixed first (`ec600e8a`); the
/// other two, plus `CANDIDATE_MAGIC`, `POLICY_MAGIC` and `STATE_MAGIC`, were
/// still bare when this gate was written. Nothing was red the whole time.
///
/// **No exemption register, deliberately.** A magic VALUE is a shipped wire
/// discriminant, so re-lettering one is a wire event needing a decision record
/// -- which is why [`check`] must be able to adjudicate rather than demand.
/// A constant NAME is not on the wire at all. Renaming one costs a
/// re-emission and an import, and never a redeploy, so there is nothing here
/// to trade off and no argued verdict that could excuse it. Fix it at the
/// author: for a Lean-emitted constant that is the emitter, not the generated
/// file the emitter overwrites.
///
/// Returns the gate failures and, separately, the name-shares where fewer than
/// two of the claimants are exported. A file-private `const MAGIC` cannot be
/// named from anywhere else, so it can mislead no importer; it is printed for
/// the same reason a mirror is, and does not fail the gate.
pub fn check_names(declared: &[DeclaredMagic]) -> (Vec<String>, Vec<String>) {
    let mut by_name: BTreeMap<&str, BTreeMap<&str, Vec<&DeclaredMagic>>> = BTreeMap::new();
    for magic in declared {
        by_name
            .entry(&magic.name)
            .or_default()
            .entry(&magic.value)
            .or_default()
            .push(magic);
    }

    let mut problems = Vec::new();
    let mut unexported = Vec::new();

    for (name, by_value) in &by_name {
        if by_value.len() < 2 {
            continue;
        }
        let exported: Vec<&&str> = by_value
            .iter()
            .filter(|(_, sites)| sites.iter().any(|site| site.exported))
            .map(|(value, _)| value)
            .collect();
        if exported.len() < 2 {
            let values: Vec<&str> = by_value.keys().copied().collect();
            unexported.push(format!(
                "{name} = {} across {} declarations, {} exported",
                values.join(" / "),
                by_value.values().map(Vec::len).sum::<usize>(),
                exported.len()
            ));
            continue;
        }
        let sites = by_value
            .iter()
            .flat_map(|(value, entries)| {
                entries
                    .iter()
                    .filter(|entry| entry.exported)
                    .map(move |entry| format!("{value} ({})", entry.provenance))
            })
            .collect::<Vec<_>>()
            .join(", ");
        problems.push(format!(
            "constant `{name}` is exported for {} different magics: {sites}. A name is what a \
             consumer writes, so two values behind one spelling means an import cannot say which \
             eight bytes it carries, and a name index refuses it rather than answering it. \
             Unlike a magic value, a name is not on the wire: rename it -- at the emitter, if it \
             is emitted -- and give each family its own prefix. There is no register to \
             adjudicate this into, because there is nothing to trade off.",
            exported.len()
        ));
    }

    (problems, unexported)
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
            exported: true,
        }
    }

    /// The same declaration written as a file-private `const`.
    fn private(package: &str, name: &str, value: &str, provenance: &str) -> DeclaredMagic {
        DeclaredMagic {
            exported: false,
            ..magic(package, name, value, provenance)
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
                "dclutch-trading",
                "DIRECT_REPLAY_SETUP_REQUEST_MAGIC_V1",
                "DCLTDRS1",
                "crates/dclutch-trading/src/replay_setup_v1.rs:13",
            ),
        ];
        let (problems, summary) = check(&declared, &[]);
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
        let (problems, summary) = check(&declared, &[]);
        assert!(problems.is_empty(), "a mirror is not a gate failure");
        assert_eq!(summary.mirrored.len(), 1);
        assert!(summary.mirrored[0].contains("DCLTPCA1"));
    }

    fn exemption(magic: &str, constants: &[&str], verdict: &str) -> Exemption {
        Exemption {
            magic: magic.into(),
            constants: constants.iter().map(|c| (*c).to_string()).collect(),
            verdict: verdict.into(),
            owner: "a named owner".into(),
        }
    }

    const GOOD_VERDICT: &str = "DIFFERENT ELFS. Neither program's entrypoint ever sees the \
         other's constant, so no single dispatcher has to choose between them and no \
         wrong-handler path exists.";

    fn colliding() -> [DeclaredMagic; 2] {
        [
            magic("a", "A_REQUEST_MAGIC_V1", "DCLTXXX1", "a.rs:1"),
            magic("b", "B_REQUEST_MAGIC_V1", "DCLTXXX1", "b.rs:1"),
        ]
    }

    /// The register excuses a collision only when the verdict is argued.
    #[test]
    fn an_argued_exemption_for_the_exact_set_excuses_the_collision() {
        let ex = [exemption(
            "DCLTXXX1",
            &["A_REQUEST_MAGIC_V1", "B_REQUEST_MAGIC_V1"],
            GOOD_VERDICT,
        )];
        let (problems, summary) = check(&colliding(), &ex);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(summary.exempted, 1);
    }

    /// The whole point of the register: silence has to be bought with an
    /// argument. An entry that records nothing is itself a gate failure.
    #[test]
    fn an_exemption_with_no_argued_verdict_is_refused_not_honoured() {
        for empty in ["", "   ", "safe", "n/a -- checked, it is fine"] {
            let ex = [exemption(
                "DCLTXXX1",
                &["A_REQUEST_MAGIC_V1", "B_REQUEST_MAGIC_V1"],
                empty,
            )];
            let (problems, summary) = check(&colliding(), &ex);
            assert_eq!(summary.exempted, 0, "an unargued entry may not excuse");
            assert!(
                problems.iter().any(|p| p.contains("no argued verdict")),
                "verdict {empty:?} must be refused, got {problems:?}"
            );
            assert!(
                problems.iter().any(|p| p.contains("claimed by")),
                "and the collision itself must still be reported"
            );
        }
    }

    /// An exemption pins a fact, it does not silence a value. A third claimant
    /// is a NEW collision and may not inherit the old verdict.
    #[test]
    fn a_new_claimant_breaks_the_exemption_rather_than_inheriting_it() {
        let mut declared = colliding().to_vec();
        declared.push(magic("c", "C_REQUEST_MAGIC_V1", "DCLTXXX1", "c.rs:1"));
        let ex = [exemption(
            "DCLTXXX1",
            &["A_REQUEST_MAGIC_V1", "B_REQUEST_MAGIC_V1"],
            GOOD_VERDICT,
        )];
        let (problems, summary) = check(&declared, &ex);
        assert_eq!(summary.exempted, 0);
        assert!(
            problems.iter().any(|p| p.contains("A new claimant")),
            "{problems:?}"
        );
    }

    /// A register of facts may not carry a stale one. This is what would have
    /// caught a `DCLTDRS1` entry surviving its own re-lettering.
    #[test]
    fn an_exemption_whose_collision_is_gone_is_reported_stale() {
        let declared = [magic("a", "A_REQUEST_MAGIC_V1", "DCLTXXX1", "a.rs:1")];
        let ex = [exemption(
            "DCLTXXX1",
            &["A_REQUEST_MAGIC_V1", "B_REQUEST_MAGIC_V1"],
            GOOD_VERDICT,
        )];
        let (problems, _) = check(&declared, &ex);
        assert!(
            problems.iter().any(|p| p.contains("no longer collides")),
            "{problems:?}"
        );
    }

    /// One constant declared once is the ordinary case and must stay silent.
    #[test]
    fn a_unique_magic_raises_nothing() {
        let declared = [magic("p", "A_MAGIC_V1", "DCLTAAA1", "a.rs:1")];
        let (problems, summary) = check(&declared, &[]);
        assert!(problems.is_empty());
        assert!(summary.mirrored.is_empty());
        assert_eq!(summary.distinct, 1);
    }

    /// The parser must take BOTH idioms -- the hand-written `*b"..."` and the
    /// emitted hex array -- and must not take a differently-sized or
    /// differently-typed constant that happens to sit next to one.
    ///
    /// The hex-array case is the one this gate was blind to. `EMITTED_MAGIC_V1`
    /// below is `DCLTEMIT` written the way a Lean emitter writes it, and it must
    /// arrive at the same rendered value as a byte string would, because the
    /// whole point is that a magic is one object however it is spelled.
    #[test]
    fn both_magic_idioms_are_swept_and_nothing_else_is() {
        let source = r#"
            pub const GOOD_MAGIC_V1: [u8; 8] = *b"DCLTGOOD";
            pub const EMITTED_MAGIC_V1: [u8; 8] =
                [0x44, 0x43, 0x4c, 0x54, 0x45, 0x4d, 0x49, 0x54];
            pub const SHORT_MAGIC: [u8; 4] = *b"DCLT";
            pub const SHORT_EMITTED: [u8; 4] = [0x44, 0x43, 0x4c, 0x54];
            pub const NOT_BYTES: [u8; 8] = [0; 8];
            pub const COMPUTED: [u8; 8] = [BASE, 1, 2, 3, 4, 5, 6, 7];
            pub const WIDTH: usize = 8;
            mod inner {
                pub const NESTED_MAGIC_V1: [u8; 8] = *b"DCLTNEST";
            }
        "#;
        let parsed = syn::parse_file(source).expect("parses");
        let mut found = Vec::new();
        collect(&parsed.items, "p", "a.rs", source, &mut found);
        let names: Vec<&str> = found.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(
            names,
            ["GOOD_MAGIC_V1", "EMITTED_MAGIC_V1", "NESTED_MAGIC_V1"]
        );
        assert_eq!(found[0].value, "DCLTGOOD");
        assert_eq!(
            found[1].value, "DCLTEMIT",
            "an emitted hex array renders exactly as its byte-string twin would"
        );
        assert_eq!(found[2].value, "DCLTNEST", "a magic inside a module counts");
    }

    /// A hand-written magic and an emitted one carrying the SAME bytes are one
    /// collision, not two declarations the gate never compares. This is the
    /// shape of the defect the blind spot was hiding.
    #[test]
    fn a_byte_string_and_a_hex_array_collide_with_each_other() {
        let source = r#"
            pub const WRITTEN_MAGIC_V1: [u8; 8] = *b"DCLTSAME";
            pub const EMITTED_MAGIC_V1: [u8; 8] =
                [0x44, 0x43, 0x4c, 0x54, 0x53, 0x41, 0x4d, 0x45];
        "#;
        let parsed = syn::parse_file(source).expect("parses");
        let mut found = Vec::new();
        collect(&parsed.items, "p", "a.rs", source, &mut found);
        let (problems, summary) = check(&found, &[]);
        assert_eq!(summary.distinct, 1, "both declare the same wire value");
        assert_eq!(problems.len(), 1);
        assert!(
            problems[0].contains("DCLTSAME"),
            "the collision names the value: {}",
            problems[0]
        );
    }

    /// The measured case `check_names` exists for, as the tree actually
    /// declared it: two Lean emitters printing a bare `REQUEST_MAGIC` for two
    /// different wire values. Neither is a mirror (the bytes differ) and
    /// neither is a value collision (the values are unique), so BOTH existing
    /// gates were green while a name meant two things.
    #[test]
    fn two_magics_under_one_exported_name_is_a_collision() {
        let declared = [
            magic(
                "dclutch-trading",
                "REQUEST_MAGIC",
                "DCDREQ01",
                "crates/dclutch-trading/src/dealer/generated_dealer_liquidity.rs:25",
            ),
            magic(
                "dclutch-trading",
                "REQUEST_MAGIC",
                "DCGREQ01",
                "crates/dclutch-trading/src/general_codec/generated_general_controller.rs:29",
            ),
        ];
        let (value_problems, summary) = check(&declared, &[]);
        assert!(
            value_problems.is_empty(),
            "two distinct values are not a value collision: {value_problems:?}"
        );
        assert!(
            summary.mirrored.is_empty(),
            "different bytes under one name is not a mirror either"
        );

        let (problems, unexported) = check_names(&declared);
        assert_eq!(problems.len(), 1, "one name, reported once");
        assert!(problems[0].contains("REQUEST_MAGIC"), "{}", problems[0]);
        assert!(problems[0].contains("DCDREQ01"), "{}", problems[0]);
        assert!(problems[0].contains("DCGREQ01"), "{}", problems[0]);
        assert!(
            unexported.is_empty(),
            "both declarations are exported, so neither is excused"
        );
    }

    /// A name shared only by file-private constants misleads nobody: no `use`
    /// can reach it, so no import can carry the wrong bytes under it. Counted
    /// and printed, exactly as a mirror is, and NOT a gate failure. This test
    /// is the record of that decision -- `ORDER_MAGIC` (`DCGORD01`/`DCGORD02`,
    /// both private, both in `dclutch-trading::general`) is the tree's
    /// live instance, and anyone who makes this fail has to delete an
    /// assertion saying why it did not.
    #[test]
    fn a_name_shared_only_by_unexported_declarations_does_not_fail_the_gate() {
        let declared = [
            private(
                "dclutch-trading",
                "ORDER_MAGIC",
                "DCGORD01",
                "crates/dclutch-trading/src/general/collection_v1.rs:74",
            ),
            private(
                "dclutch-trading",
                "ORDER_MAGIC",
                "DCGORD02",
                "crates/dclutch-trading/src/general/runtime_manifest.rs:17",
            ),
        ];
        let (problems, unexported) = check_names(&declared);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(unexported.len(), 1);
        assert!(unexported[0].contains("ORDER_MAGIC"), "{}", unexported[0]);

        // One export among them is still only one reachable name, so it stays
        // out of the gate; a SECOND export is the collision.
        let mut one_public = declared.to_vec();
        one_public[0].exported = true;
        assert!(check_names(&one_public).0.is_empty(), "one export is safe");
        let mut both_public = one_public.clone();
        both_public[1].exported = true;
        assert_eq!(
            check_names(&both_public).0.len(),
            1,
            "the second export is what makes the name ambiguous to an importer"
        );
    }

    /// One name, one magic, however many places declare it, is the ordinary
    /// case and must stay silent -- a same-name/same-value re-declaration is a
    /// mirror, which `check` already counts and which this gate must not
    /// double-report.
    #[test]
    fn one_name_bound_to_one_magic_raises_nothing() {
        let declared = [
            magic("a", "SHARED_MAGIC_V1", "DCLTSHR1", "a.rs:1"),
            magic("b", "SHARED_MAGIC_V1", "DCLTSHR1", "b.rs:1"),
        ];
        let (problems, unexported) = check_names(&declared);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(unexported.is_empty(), "{unexported:?}");
    }

    /// The sweep has to be able to tell an export from a private constant, or
    /// the distinction the gate rests on is decided by a field nobody sets.
    #[test]
    fn the_sweep_records_whether_a_magic_is_exported() {
        let source = r#"
            pub const PUBLIC_MAGIC_V1: [u8; 8] = *b"DCLTPUB1";
            pub(crate) const CRATE_MAGIC_V1: [u8; 8] = *b"DCLTCRT1";
            const PRIVATE_MAGIC_V1: [u8; 8] = *b"DCLTPRV1";
        "#;
        let parsed = syn::parse_file(source).expect("parses");
        let mut found = Vec::new();
        collect(&parsed.items, "p", "a.rs", source, &mut found);
        let seen: Vec<(&str, bool)> = found
            .iter()
            .map(|m| (m.name.as_str(), m.exported))
            .collect();
        assert_eq!(
            seen,
            [
                ("PUBLIC_MAGIC_V1", true),
                ("CRATE_MAGIC_V1", true),
                ("PRIVATE_MAGIC_V1", false),
            ],
            "`pub(crate)` is an export -- it is exactly how every Lean emitter              in this tree writes a magic"
        );
    }
}
