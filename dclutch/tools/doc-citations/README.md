# doc-citations — ledger §8.2, "the citation outlives the thing cited"

A doc comment that names a symbol in backticks is a pointer, and pointers rot
silently. Nothing in a Rust build checks that `funded::process_funded_transition`
still exists, so when the function goes the sentence stays, keeps its authority,
and the next reader reasons from it. **Nothing goes red.**

This tree paid for it on 2026-09-01. Five comments across four files cited that
exact symbol in the *present tense* as the justification for a live on-chain
refusal (`CoreSbfError::RecoveryWalkUnavailable`, thrown at
`programs/dclutch-core-sbf/src/resolution.rs:726`), and one of them was a
*lifting plan* instructing someone to resurrect it. The function has no
definition anywhere in the tree. The lane that found this had already misreported
once from the same source, reading a deleted route's nouns as live vocabulary.

```sh
python3 tools/doc-citations/doc_citations.py --root .
python3 tools/doc-citations/doc_citations.py --root . \
    --baseline tools/doc-citations/baseline.json --check   # tripwire
```

## What it is not

It is not name resolution. No rustc, no type information, no imports, no glob
or macro expansion. It reads source text, needs no build, and runs in about
seven seconds over 1,143 files — the same posture as `tools/seam-audit`. (It
took two and a half minutes until the walk pruned `target/` instead of
filtering it afterwards; a checker nobody will wait for is a checker nobody
runs.)

## What it therefore does

It **judges the signal and declines the noise, and says which is which.**

- **Judged**: a namespaced path — `a::b`, `Type::method`, `crate::mod::item` —
  whose leading segment belongs to this workspace. Resolved if the final
  segment is declared anywhere in the tree; **unresolved** otherwise.
- **Declined**: prose in backticks, file paths, code fragments, and any path
  rooted in a crate whose items we cannot see.

A declined citation is **not a passing citation.** It is one the tool refuses to
have an opinion about, and the count is printed so nobody mistakes silence for
coverage. Today: 11,862 backtick spans, 1,031 judged, 10,831 declined.

The index covers items, **enum variants and struct fields**. Variants and fields
are in there because docs cite them constantly as `Type::Variant` and
`Type::field` — without them the report is nothing but those, and a report that
is mostly noise is one nobody reads twice. Adding fields alone took the finding
count from 24 to 11.

## The two false-positive classes, with today's examples

Precision is about half, and that is the intended trade: **a false positive costs
a second to dismiss; a false negative cost this tree a day.**

1. **Foreign head we happen to also declare.** `Result::expect_err`,
   `Rent::from_account_info`, `ComputeBudget::RequestHeapFrame`,
   `PoolState::INIT_SPACE`. The head is external, but some crate here declares a
   type of the same name, so the path is judged instead of declined. Five
   citations today.
2. **Illustrative paths in prose.** `a::is_x` in
   `tools/gauntlet/census/src/enumerate.rs:49` is an example of path
   normalisation, not a citation. One today.

A third class exists and is deliberately not suppressed: a comment that
*correctly reports* a symbol as missing still cites it.
`programs/dclutch-resolution-proof-sbf/src/funded.rs:27` is exactly that — the
note recording that four other comments dangle. The tool cannot tell a warning
from a claim, and should not guess.

## Owed, as of 2026-09-01

Both true findings are in the baseline so the tripwire works; **the baseline is a
line, not an absolution.**

- `funded::process_funded_transition` ×4 — `core-sbf/src/lib.rs:149`,
  `resolution.rs:865`, `resolution.rs:880`, and the meta-note in
  `resolution-proof-sbf/src/funded.rs:27`. `:880` is the lifting plan and
  attaches to the open **"recovery ontology: keep or cut"** ruling rather than
  to an edit. The live half of what these justify still holds:
  `exhaust_after_primary_deadline` refuses `recovery_policy().is_some()`
  outright, so the refusal they argue for is correct even though its stated
  reason names vanished symbols.
- `ClusterOriginV1::may_use_seeded_keys` —
  `tools/local-validator/bootstrap/successor/src/cluster.rs:49`. Written as a
  rustdoc intra-doc link. No such method; the live function is the free
  `seeded_keys_admissible` at `cluster.rs:457`.

## Controls

`./negative-control.sh` proves all three directions, because a checker that
cannot fail is indistinguishable from a clean tree and a tripwire that cannot
fire is worse than none:

1. this tree still reports the citation the tool was built from;
2. a synthetic tree resolves items, enum variants and struct fields, and
   reports the one absent symbol;
3. `--check` exits nonzero on a citation absent from the baseline.

Control 2 runs in a temp directory on purpose — a shared checkout must not have
a control injecting doc comments into another lane's file. It earned its keep
immediately: it caught the indexer missing members of single-line
`enum E { A }` and `struct S { a: u8 }` declarations, and then items declared
after `{` on a one-line `mod`. Both are fixed. The widening was checked against
the real tree the honest way round — the finding count held at 11, so a more
permissive index did not quietly resolve the signal away.
