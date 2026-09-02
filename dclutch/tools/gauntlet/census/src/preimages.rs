//! Every schema identity the tree claims to be the SHA-256 of a label it ships.
//!
//! # The claim nobody was checking
//!
//! The tree's identity idiom is a pair. A `&[u8]` label, and next to it the
//! 32 bytes of its digest, with a doc comment on the digest saying which label
//! it came from:
//!
//! ```text
//! /// Schema label for finalized `ExecutionStrategyCertificateV2` records.
//! pub const EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_PREIMAGE_V2: &[u8] =
//!     b"dclutch/schema/execution-strategy-certificate-v2";
//! /// SHA-256 of [`EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_PREIMAGE_V2`].
//! pub const EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2: [u8; 32] = [ .. ];
//! ```
//!
//! That doc comment is a load-bearing assertion -- the identity is what a
//! program compares a record's schema field against, so a wrong digest either
//! refuses everything or accepts the wrong thing -- and until this module it
//! was checked by nothing. Measured 2026-09-02, before this gate existed: 130
//! `*PREIMAGE*` labels declared, 99 of them carrying such a claim, and **no
//! test, tool, or gate anywhere in the tree recomputed a single one**. The
//! `sha2` dependency this uses was already in the census manifest.
//!
//! The census hashes nothing else, so this is a small addition rather than a
//! new capability: it reads what the tree already wrote down and does the
//! arithmetic the comment promises.
//!
//! # Why the doc comment is the pairing authority, not the name
//!
//! A name rule (`X_PREIMAGE` pairs with `X_ID`) would be a second author for a
//! fact the file already states. The file's own `[`NAME`]` reference is the
//! statement, so that is what is read -- and it is complete: of the 99 pairs,
//! **99 are found by the doc reference and 0 need a name rule**, and no digest
//! doc names a label its file does not declare. A pairing invented here could
//! silently pair the wrong two constants; one read out of the source cannot.
//!
//! # What is not a failure
//!
//! A label with no digest constant beside it. Thirty-one of them exist, and
//! they are not an omission: their consumers hash the label at run time
//! (`hash(SERIES_SUCCESSOR_KIND_PREIMAGE_V3)`,
//! `Sha256::digest(DEALER_CONFIG_SCHEMA_PREIMAGE_V4)`) rather than comparing
//! against a pinned constant, so there is no claim to check. They are counted
//! and reported so the number is visible, never failed.
//!
//! `#[cfg(test)]` modules are skipped. A fixture that declares a deliberately
//! wrong digest is a hostile, which is the opposite of a defect, and this gate
//! must not make writing one impossible.

use std::{collections::BTreeMap, fs, path::Path};

use sha2::{Digest, Sha256};
use syn::{Attribute, Expr, Item, Lit, Meta};

use crate::enumerate::rust_sources;

/// One `const NAME: &[u8] = b"...";` whose identifier names it a preimage.
#[derive(Clone, Debug)]
struct Label {
    bytes: Vec<u8>,
}

/// One `const NAME: [u8; 32] = [ .. ];` and the doc comment above it.
#[derive(Clone, Debug)]
struct Identity {
    name: String,
    bytes: [u8; 32],
    doc: String,
    provenance: String,
}

/// One digest constant whose doc says it is the SHA-256 of a label the same
/// file declares.
#[derive(Clone, Debug)]
pub struct DeclaredPair {
    pub label: String,
    pub identity: String,
    /// `path:line` of the digest constant, repo-relative.
    pub provenance: String,
    /// SHA-256 of the label, as the tree's own bytes would have to be.
    computed: [u8; 32],
    declared: [u8; 32],
}

impl DeclaredPair {
    fn holds(&self) -> bool {
        self.computed == self.declared
    }
}

fn render(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The `///` lines above an item, joined.
fn doc_of(attrs: &[Attribute]) -> String {
    let mut text = String::new();
    for attr in attrs {
        let Meta::NameValue(pair) = &attr.meta else {
            continue;
        };
        if !pair.path.is_ident("doc") {
            continue;
        }
        if let Expr::Lit(literal) = &pair.value
            && let Lit::Str(line) = &literal.lit
        {
            text.push_str(&line.value());
            text.push('\n');
        }
    }
    text
}

/// Whether a type is exactly `&[u8]`.
fn is_byte_slice(ty: &syn::Type) -> bool {
    let syn::Type::Reference(reference) = ty else {
        return false;
    };
    let syn::Type::Slice(slice) = reference.elem.as_ref() else {
        return false;
    };
    matches!(slice.elem.as_ref(), syn::Type::Path(path) if path.path.is_ident("u8"))
}

/// Whether a type is exactly `[u8; 32]`.
fn is_digest_array(ty: &syn::Type) -> bool {
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
    matches!(&literal.lit, Lit::Int(int) if int.base10_parse::<usize>().is_ok_and(|n| n == 32))
}

/// The bytes behind `b"...."`.
fn byte_string(expr: &Expr) -> Option<Vec<u8>> {
    match expr {
        Expr::Lit(literal) => match &literal.lit {
            Lit::ByteStr(bytes) => Some(bytes.value()),
            _ => None,
        },
        _ => None,
    }
}

/// The 32 bytes behind `[0x01, 0x02, ..]`, only when every element is a literal.
fn digest_array(expr: &Expr) -> Option<[u8; 32]> {
    let Expr::Array(array) = expr else {
        return None;
    };
    if array.elems.len() != 32 {
        return None;
    }
    let mut out = [0u8; 32];
    for (slot, element) in out.iter_mut().zip(array.elems.iter()) {
        let Expr::Lit(literal) = element else {
            return None;
        };
        let Lit::Int(int) = &literal.lit else {
            return None;
        };
        *slot = int.base10_parse::<u8>().ok()?;
    }
    Some(out)
}

/// Whether an item carries `#[cfg(test)]`.
fn is_test_gated(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("cfg")
            && attr
                .meta
                .require_list()
                .is_ok_and(|list| list.tokens.to_string().replace(' ', "") == "test")
    })
}

fn line_of(text: &str, needle: &str) -> usize {
    text.find(needle).map_or(0, |offset| {
        text[..offset].bytes().filter(|b| *b == b'\n').count() + 1
    })
}

fn collect(
    items: &[Item],
    relative: &str,
    source: &str,
    labels: &mut BTreeMap<String, Label>,
    identities: &mut Vec<Identity>,
) {
    for item in items {
        match item {
            Item::Mod(module) if !is_test_gated(&module.attrs) => {
                if let Some((_, inner)) = &module.content {
                    collect(inner, relative, source, labels, identities);
                }
            }
            Item::Const(konst) if !is_test_gated(&konst.attrs) => {
                let name = konst.ident.to_string();
                if name.contains("PREIMAGE")
                    && is_byte_slice(&konst.ty)
                    && let Some(bytes) = byte_string(&konst.expr)
                {
                    labels.insert(name, Label { bytes });
                } else if is_digest_array(&konst.ty)
                    && let Some(bytes) = digest_array(&konst.expr)
                {
                    identities.push(Identity {
                        doc: doc_of(&konst.attrs),
                        provenance: format!(
                            "{relative}:{}",
                            line_of(source, &format!("const {name}"))
                        ),
                        name,
                        bytes,
                    });
                }
            }
            _ => {}
        }
    }
}

/// Whether a doc comment asserts a SHA-256 derivation at all.
fn asserts_sha256(doc: &str) -> bool {
    let upper = doc.to_ascii_uppercase();
    upper.contains("SHA-256") || upper.contains("SHA256")
}

/// The `[`NAME`]` intra-doc references in a doc comment.
fn referenced_names(doc: &str) -> Vec<String> {
    let mut found = Vec::new();
    let bytes = doc.as_bytes();
    let mut index = 0usize;
    while let Some(start) = doc[index..].find("[`") {
        let open = index + start + 2;
        let Some(length) = doc[open..].find('`') else {
            break;
        };
        let name = &doc[open..open + length];
        if !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
            && bytes.get(open + length + 1) == Some(&b']')
        {
            found.push(name.to_string());
        }
        index = open + length + 1;
    }
    found
}

/// Every claim one source file makes, and every label in it that makes none.
///
/// Split out of [`sweep`] so the rule can be exercised on a source string: the
/// pairing is the part that could be wrong, and it should not need a checkout
/// to test.
fn scan_source(text: &str, relative: &str) -> (Vec<DeclaredPair>, Vec<String>) {
    let mut pairs = Vec::new();
    let mut unpaired = Vec::new();
    let Ok(parsed) = syn::parse_file(text) else {
        return (pairs, unpaired);
    };
    let mut labels = BTreeMap::new();
    let mut identities = Vec::new();
    collect(&parsed.items, relative, text, &mut labels, &mut identities);

    let mut claimed: BTreeMap<String, &Identity> = BTreeMap::new();
    for identity in &identities {
        if !asserts_sha256(&identity.doc) {
            continue;
        }
        for name in referenced_names(&identity.doc) {
            if labels.contains_key(&name) {
                claimed.entry(name).or_insert(identity);
            }
        }
    }
    for (name, label) in &labels {
        match claimed.get(name) {
            Some(identity) => pairs.push(DeclaredPair {
                label: name.clone(),
                identity: identity.name.clone(),
                provenance: identity.provenance.clone(),
                computed: Sha256::digest(&label.bytes).into(),
                declared: identity.bytes,
            }),
            None => unpaired.push(format!(
                "{name} ({relative}:{})",
                line_of(text, &format!("const {name}"))
            )),
        }
    }
    (pairs, unpaired)
}

/// Every declared preimage/identity pair under the protocol packages.
///
/// Returns the claimed pairs, and the labels that claim nothing rendered as
/// `LABEL (path:line)` -- a reported value, never a failing one, so it takes
/// the same shape `MagicSummary::mirrored` uses for the same reason.
pub fn sweep(root: &Path) -> Result<(Vec<DeclaredPair>, Vec<String>), String> {
    let owners = super::bands::package_directories(root)?;
    let mut pairs = Vec::new();
    let mut unpaired = Vec::new();
    for (package, directory) in &owners {
        for path in rust_sources(directory)? {
            // Innermost-first, exactly as `magics::sweep` does it: the first
            // package whose directory contains the file is the one that
            // compiles it, so a nested program-test is not counted twice.
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
            if !text.contains("PREIMAGE") {
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            let (found, missing) = scan_source(&text, &relative);
            pairs.extend(found);
            unpaired.extend(missing);
        }
    }
    pairs.sort_by(|left, right| left.label.cmp(&right.label));
    unpaired.sort();
    Ok((pairs, unpaired))
}

pub struct PreimageSummary {
    pub claimed: usize,
    pub verified: usize,
    pub unpaired: usize,
}

/// Every claimed derivation must hold. There is no exemption register: unlike a
/// magic collision, which can be an adjudicated fact about two dispatchers, a
/// digest that is not its label's SHA-256 is a false statement in the source
/// with no reading that makes it true.
pub fn check(pairs: &[DeclaredPair], unpaired: &[String]) -> (Vec<String>, PreimageSummary) {
    let mut problems = Vec::new();
    let mut verified = 0usize;
    for pair in pairs {
        if pair.holds() {
            verified += 1;
            continue;
        }
        problems.push(format!(
            "`{}` ({}) documents itself as the SHA-256 of `{}`, and it is not: the label hashes \
             to {}, the constant declares {}. Fix the constant, or fix the label -- but the \
             comment and the bytes cannot both stand. A schema identity is what a program \
             compares a record's field against, so a wrong one refuses every honest record or \
             admits a foreign one.",
            pair.identity,
            pair.provenance,
            pair.label,
            render(&pair.computed),
            render(&pair.declared)
        ));
    }
    let summary = PreimageSummary {
        claimed: pairs.len(),
        verified,
        unpaired: unpaired.len(),
    };
    (problems, summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SHA-256 of `b"dclutch/schema/example-v1"`, as `sha2` computes it. The
    /// source under test states it the way the tree states every identity.
    const EXAMPLE: &str = r#"
/// Schema label for the worked example.
pub const EXAMPLE_SCHEMA_PREIMAGE_V1: &[u8] = b"dclutch/schema/example-v1";
/// SHA-256 of [`EXAMPLE_SCHEMA_PREIMAGE_V1`].
pub const EXAMPLE_SCHEMA_ID_V1: [u8; 32] = [PLACEHOLDER];
"#;

    fn rendered(bytes: &[u8]) -> String {
        bytes
            .iter()
            .map(|byte| format!("0x{byte:02x}"))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The example source with `id` as the declared digest.
    fn source_with(id: &[u8; 32]) -> String {
        EXAMPLE.replace("PLACEHOLDER", &rendered(id))
    }

    fn truth() -> [u8; 32] {
        Sha256::digest(b"dclutch/schema/example-v1").into()
    }

    #[test]
    fn a_documented_identity_that_is_its_label_s_digest_passes() {
        let (pairs, unpaired) = scan_source(&source_with(&truth()), "example.rs");
        assert_eq!(unpaired.len(), 0, "the label is paired: {unpaired:?}");
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].label, "EXAMPLE_SCHEMA_PREIMAGE_V1");
        assert_eq!(pairs[0].identity, "EXAMPLE_SCHEMA_ID_V1");
        let (problems, summary) = check(&pairs, &unpaired);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(summary.verified, 1);
    }

    /// The case the gate exists for, and the one the tree could not see before
    /// it: the comment says SHA-256, the bytes are something else.
    #[test]
    fn one_wrong_byte_in_the_digest_is_a_gate_failure_naming_both_values() {
        let mut wrong = truth();
        wrong[0] ^= 0x01;
        let (pairs, unpaired) = scan_source(&source_with(&wrong), "example.rs");
        assert_eq!(pairs.len(), 1, "still a claimed pair");
        let (problems, summary) = check(&pairs, &unpaired);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert_eq!(summary.verified, 0);
        assert!(problems[0].contains(&render(&truth())), "{}", problems[0]);
        assert!(problems[0].contains(&render(&wrong)), "{}", problems[0]);
        assert!(problems[0].contains("example.rs:"), "{}", problems[0]);
    }

    /// Thirty of the tree's labels have no digest beside them because their
    /// consumers hash at run time. Reported, never failed.
    #[test]
    fn a_label_with_no_digest_constant_is_reported_and_does_not_fail() {
        let source = "pub const LONE_SCHEMA_PREIMAGE_V1: &[u8] = b\"dclutch/schema/lone-v1\";";
        let (pairs, unpaired) = scan_source(source, "lone.rs");
        assert!(pairs.is_empty());
        assert_eq!(unpaired.len(), 1);
        assert!(unpaired[0].starts_with("LONE_SCHEMA_PREIMAGE_V1 (lone.rs:1)"));
        let (problems, summary) = check(&pairs, &unpaired);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(summary.unpaired, 1);
    }

    /// A digest that documents no derivation makes no claim, so pairing it
    /// would be this module inventing the fact rather than reading it.
    #[test]
    fn a_digest_whose_doc_claims_no_derivation_creates_no_pair() {
        let source = source_with(&truth()).replace(
            "/// SHA-256 of [`EXAMPLE_SCHEMA_PREIMAGE_V1`].",
            "/// An identity chosen by the Registry, related to [`EXAMPLE_SCHEMA_PREIMAGE_V1`].",
        );
        let (pairs, unpaired) = scan_source(&source, "example.rs");
        assert!(pairs.is_empty(), "no SHA-256 claim, no pair");
        assert_eq!(unpaired.len(), 1);
    }

    /// A hostile fixture may declare a deliberately wrong digest; that is a
    /// test of the protocol, not a defect in it.
    #[test]
    fn a_cfg_test_module_is_not_swept() {
        let mut wrong = truth();
        wrong[0] ^= 0x01;
        let source = format!("#[cfg(test)]\nmod fixtures {{{}}}", source_with(&wrong));
        let (pairs, unpaired) = scan_source(&source, "example.rs");
        assert!(pairs.is_empty(), "{pairs:?}");
        assert!(unpaired.is_empty(), "{unpaired:?}");
    }

    /// Only a bare screaming-case reference is a name in this file. A path
    /// reference names something else's constant, which this module cannot
    /// hash and must not guess at.
    #[test]
    fn only_bare_uppercase_intra_doc_references_are_read_as_local_names() {
        assert_eq!(
            referenced_names("SHA-256 of [`A_PREIMAGE_V1`] and [`crate::v2::B_PREIMAGE_V1`]."),
            vec!["A_PREIMAGE_V1".to_string()]
        );
        assert!(referenced_names("see [`SomeType`] and [`lower_case`]").is_empty());
    }
}
