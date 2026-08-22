# Codex handoff — historical Cycle-G optimization session

> Superseded for current work on 2026-08-22 by
> [`CURRENT_TRUTH.md`](CURRENT_TRUTH.md) and the
> [`architecture review`](docs/reviews/ARCHITECTURE_REVIEW_2026-08-22.md).
> The measurements and gate pointers below remain useful. Its
> “capability-complete” phrase names the bounded Cycle-G matrix, not the full
> recovered product thesis, every collateral profile, or every settlement
> shape.

Rewritten 2026-08-22 (the prior version pinned an identity five seals old).
Read order: this file → `MACRO_AND_MICRO_OPTIMIZATION.md` (your work list)
→ `CURRENT_TRUTH.md` §§1-3 (the claim vocabulary and live state).

## State in one paragraph

The bounded Cycle-G matrix is capability-complete: the general clearing plane runs
place → freeze → walk → verify → select → entitle → settle with partial
fills, realized rounding/virtual pots, and an on-chain no-arbitrage gate
for degree-2/3 claims; every account has a close path returning exact
recorded principal; fees are computable (rates deliberately zero); two
source generations exist and the default ELF takes custody against a
registered pull release; a keeper cranks the whole lifecycle
permissionlessly; a browser bench trades it human-vs-automaton.
`SETTLEMENT_BLOCKERS` is empty. Current sealed identity:
**`0d52c561909cedef…`, 2,149,672 bytes**, manifest `902e8d2`
(101 gates, 100/101 with one documented drift window), Persvati-attested.
The historical next job was CU/size optimization per
`MACRO_AND_MICRO_OPTIMIZATION.md`. The current review also reopens explicitly
named generality and product-boundary work; this handoff no longer constrains the
queue to optimization alone.

## Where you work

- **This machine, in the main checkout** (`/Users/ember/dev/dragons-clutch`)
  — which is the canonical build path, so your ELF hashes compare
  directly against the sealed identity. Two consequences: (a) if another
  agent session is active in the tree, use the shared-tree discipline —
  commit only named files, retry on index lock, and serialize full-suite
  or validator runs behind the spinlock
  (`until mkdir /tmp/claude-501/suite.lock 2>/dev/null; do sleep 20; done`
  … `rmdir /tmp/claude-501/suite.lock`); (b) long builds compete with
  ember's foreground work — `nice` heavy cargo invocations, and hbox
  remains available as overflow for bulk rebuilds if the laptop gets
  loud (`swarm-build`, co-tenant rules apply there).
- Toolchains, pinned: program workspace rustc 1.89.0 / cargo-build-sbf
  4.0.0 / platform-tools v1.53; the svm-tests workspace has its own
  1.93.1 pin (`programs/clutch-sbf/svm-tests/rust-toolchain.toml`).
  Verify both build green before changing anything: one filtered test
  per workspace, never a bare `-p` suite (house rule; measured 9.5 GB /
  2 h of waste the time it was broken).

## Working agreement

1. **Branches, not main**: `codex/opt-<topic>`, pushed, merged from the
   laptop side after review. (We just deleted 27 dead branches after a
   full patch-id triage — keep the tree that clean: one branch per
   topic, delete on merge.)
2. **Every optimization commit carries its measurements in the body**:
   route, CU before → after at your path, the campaign that produced
   them, and the gates run. A number without its context is a rumor.
3. **The gates that must stay green** for any change:
   the touched plane's conservation + hostile batteries; the frame
   probe (zero `clutch_sbf` diagnostics is a sealed invariant); clippy
   `-D warnings`; the equivalence corpora where the relation is touched
   (the 2,592-book batch/stream corpus, the 322-point resume corpus).
4. **Never touch**: the frozen policy consts' bytes/digests
   (`GENERAL_CLEARING_POLICY_V1` and siblings), the exact-integer
   settlement discipline, refusal semantics, account formats without a
   version bump, `research/liveness-policy-profile/artifacts/` (sealed
   evidence), `MANIFEST.baseline.json`.
5. **rustfmt named files only** — the tree is not fmt-clean and a wide
   fmt tramples concurrent work.
6. Commit messages: lowercase-imperative subject that tells the story,
   body with the finding; end with your own attribution trailer.

## Pointers

- `MACRO_AND_MICRO_OPTIMIZATION.md` — the ranked work list with sealed
  baselines. Start at M1 (ClearWork partial-decode) or M2 (the
  EntitleSlice locator); M2 is the cleaner first win.
- `research/liveness-policy-profile/evidence.json` — the sealed CU rows
  (the W1 block: 107 shape-keyed routes) your deltas compare against.
- `programs/clutch-sbf/svm-tests/tests/scale_*.rs` — the campaign
  harness and `Meter` idiom; clone these shapes for new measurements.
- `docs/research/DUAL_IS_THE_MEASURE.md`, the partial-fill and
  operator-bench design docs in `docs/design/` — the semantics you must
  not bend for cycles.
- `BRANCH_TRIAGE_2026-08-22.md` (untracked) — why the branch namespace
  is empty; the recovery hashes if you ever need history.

The meters are honest; leave them that way.
