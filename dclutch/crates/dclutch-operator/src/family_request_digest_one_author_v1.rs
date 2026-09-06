//! One author for the accelerator family-request digest, checked over sources.
//!
//! `family_request_digest_v3` is
//! `sha256("dclutch:shadow-family-request:v3" ‖ 0x00 ‖ len_le_u32 ‖ request)`,
//! and it is the value the chain seeds every accelerator caller-authority PDA
//! with: `admitted_composition_v3.rs` and `series/shadow_operator.rs` both feed
//! `context.family_request_digest` into
//! `accelerator_caller_authority_digest_v1`. A route report that publishes a
//! bare `sha256(request)` under that name therefore names an address no
//! execution can derive. That is not hypothetical: the General route published
//! the bare hash, the operator's devnet driver seeded four child addresses from
//! it, and the transaction refused `TradingSbfError::Release` with no
//! accelerator invoke in the log.
//!
//! # Why this is a source census and not a report walk
//!
//! The natural gate is to build each family's route report and rejoin its
//! published field against the request bytes its own instruction carries.
//! `general_successor::serialize_plan_v5` is exactly that, and it is the reason
//! General's fixture was convicted. But no operator route report is
//! constructible in a `--lib` test: `build_general_hot_instruction_v3` is
//! driven only from `programs/dclutch-accelerator-sbf`'s program-test, and
//! neither Dealer builder has a caller or a full frame fixture anywhere. A
//! report walk here would have to invent the frames it claims to check.
//!
//! So this checks the property one level up, where it is exact and needs no
//! fixture: **no site in the tree initializes anything named
//! `family_request_digest` from a hashing primitive other than
//! `family_request_digest_v3`.** It costs one pass over the workspace sources
//! and it fires on a file that does not exist yet, which the two convicted
//! fixtures show is where this defect keeps appearing.
//!
//! # The discriminator, which is the part worth keeping
//!
//! `family_request_digest_v3` does NOT own every request digest on the Hot
//! path, and sweeping the others into it would break them. The Hot prelude
//! computes a SECOND, deliberately bare digest --
//! `hot_v3.rs`'s `let request_digest = hash(family_request).to_bytes()` -- and
//! carries it as `parent.parent_request_digest`, which `hot_v3/children.rs`
//! rejoins as `dclutch_sha256_adapter::digest(family_request)`. The same bare
//! form is the chain's own author for the Trading- and Claims-role child
//! caller authorities in `generic_market_founding_v1.rs` and
//! `terminal_settlement_v3.rs`. `direct_inline_route_v3`, `market_founding`,
//! `terminal_retirement_v1` and `wallet_terminal_payout_v3` all spell those by
//! hand and are all correct.
//!
//! The census therefore keys on the NAME `family_request_digest` -- the
//! accelerator concept -- and states here, rather than in a reviewer's head,
//! that `parent_request_digest` and `role_request_digest` are a different
//! digest with a different author.

use std::path::{Path, PathBuf};

/// The name whose initializers this census owns.
const FIELD: &str = "family_request_digest";
/// The one admissible author.
const AUTHOR: &str = "family_request_digest_v3(";
/// This file, which carries the detector's own bare-hash fixtures and is the
/// one source in the tree the census must not read. It publishes no route
/// report; its fixtures are covered by
/// [`the_census_convicts_the_bare_hash_and_admits_the_domain_separated_one`].
const CENSUS_SOURCE: &str = file!();
/// Hashing primitives that are a second author when they appear in an
/// initializer of [`FIELD`]. Each is a real primitive name in this tree.
const PRIMITIVES: [&str; 6] = [
    "hash(",
    "hashv(",
    "Sha256",
    "sha256",
    "digestv(",
    "digest32(",
];

/// How an initializer of [`FIELD`] spells its value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Author {
    /// A hashing primitive that is not [`AUTHOR`].
    Primitive,
    /// [`AUTHOR`].
    FamilyRequestDigestV3,
    /// Neither: a move, a test constant, a type in a field declaration.
    Neither,
}

/// The initializer's text: from just past the `:` or `=` to the end of the
/// expression, tracking bracket depth so a nested `,` does not cut it short.
fn initializer(text: &str, from: usize) -> &str {
    let bytes = text.as_bytes();
    let mut depth = 0_usize;
    let mut at = from;
    while at < bytes.len() {
        match bytes[at] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            b',' | b';' if depth == 0 => break,
            _ => {}
        }
        at += 1;
    }
    text.get(from..at).unwrap_or_default()
}

fn classify(body: &str) -> Author {
    if PRIMITIVES.iter().any(|token| body.contains(token)) {
        Author::Primitive
    } else if body.contains(AUTHOR) {
        Author::FamilyRequestDigestV3
    } else {
        Author::Neither
    }
}

/// Every initializer of [`FIELD`] in `text`, as `(line, author)`.
fn sites(text: &str) -> Vec<(usize, Author)> {
    let mut found = Vec::new();
    for (at, _) in text.match_indices(FIELD) {
        // Not the tail of a longer identifier.
        if text[..at]
            .chars()
            .next_back()
            .is_some_and(|before| before.is_alphanumeric() || before == '_')
        {
            continue;
        }
        let after = at + FIELD.len();
        let rest = text.get(after..).unwrap_or_default();
        let separator = rest.trim_start();
        let skipped = rest.len() - separator.len();
        let body_at = if let Some(tail) = separator.strip_prefix(':') {
            // A type position (`family_request_digest: [u8; 32]`) reaches here
            // too and classifies as `Neither`; it names no primitive.
            let _ = tail;
            after + skipped + 1
        } else if separator.starts_with('=') && !separator.starts_with("==") {
            after + skipped + 1
        } else {
            continue;
        };
        found.push((
            text[..at].matches('\n').count() + 1,
            classify(initializer(text, body_at)),
        ));
    }
    found
}

/// Workspace root, asserted rather than assumed.
fn workspace_root() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonicalize the workspace root");
    assert!(
        root.join("crates/dclutch-operator/Cargo.toml").is_file(),
        "the census did not find this crate under {}",
        root.display(),
    );
    root
}

fn rust_sources(at: &Path, into: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(at) else {
        return;
    };
    let mut paths = entries
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            rust_sources(&path, into);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            into.push(path);
        }
    }
}

/// The detector fires on the exact form both convicted sites carried.
///
/// Without this the census is an absent signal that cannot be told from a
/// disconnected instrument: it reports "zero bare hashes" identically whether
/// the tree is clean or the classifier is broken.
#[test]
fn the_census_convicts_the_bare_hash_and_admits_the_domain_separated_one() {
    let bare = "        family_request_digest: hash(family_request).to_bytes(),\n";
    assert_eq!(sites(bare), vec![(1, Author::Primitive)]);

    let fixture = "            family_request_digest: Sha256::digest(&request_bytes).into(),\n";
    assert_eq!(sites(fixture), vec![(1, Author::Primitive)]);

    let repaired = "        family_request_digest: family_request_digest_v3(family_request)\n            .map_err(DealerEquityHotOperatorErrorV3::ShadowDigest)?\n            .to_bytes(),\n";
    assert_eq!(sites(repaired), vec![(1, Author::FamilyRequestDigestV3)]);

    let declaration = "    pub family_request_digest: [u8; 32],\n";
    assert_eq!(sites(declaration), vec![(1, Author::Neither)]);

    // A move from an already-authored context is not an initializer of its
    // own; the trailing mention is not followed by `:` or `=` at all.
    let moved = "            family_request_digest: invocation_context.family_request_digest,\n";
    assert_eq!(sites(moved), vec![(1, Author::Neither)]);
}

/// No site in the tree spells this digest by hand.
#[test]
fn family_request_digest_v3_is_the_only_author_of_a_family_request_digest() {
    let root = workspace_root();
    let mut sources = Vec::new();
    for base in ["crates", "programs", "tools"] {
        rust_sources(&root.join(base), &mut sources);
    }
    assert!(
        sources.len() > 500,
        "the census read {} files, so it did not walk the tree",
        sources.len(),
    );

    let mut convicted = Vec::new();
    let mut authored = Vec::new();
    for path in &sources {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let relative = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .display()
            .to_string();
        if relative == CENSUS_SOURCE {
            continue;
        }
        for (line, author) in sites(&text) {
            match author {
                Author::Primitive => convicted.push(format!("{relative}:{line}")),
                Author::FamilyRequestDigestV3 => authored.push(relative.clone()),
                Author::Neither => {}
            }
        }
    }

    assert!(
        convicted.is_empty(),
        "these sites publish or seed a family request digest from a hashing \
         primitive other than `family_request_digest_v3`, which is what the \
         chain's accelerator caller authorities are seeded with: {convicted:?}",
    );

    // The second half of the positive control: the three operator family
    // builders are the subject of this census and each must still have an
    // author. A walk that read the tree but matched nothing fails here.
    for expected in [
        "crates/dclutch-operator/src/dealer_equity_hot_v3.rs",
        "crates/dclutch-operator/src/dealer_lp_hot_v4.rs",
        "crates/dclutch-operator/src/general_hot_v3.rs",
    ] {
        assert!(
            authored.iter().any(|found| found == expected),
            "{expected} no longer authors its family request digest; census: {authored:?}",
        );
    }
}
