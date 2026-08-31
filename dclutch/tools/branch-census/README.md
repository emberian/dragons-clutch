# tools/branch-census

`census.sh` answers the question `git branch --no-merged main` is usually being
asked to answer, and cannot.

## Why it exists

On 2026-08-31 `git branch --no-merged main` reported **fifty** branches in this
tree. The number of them holding work main did not already have was **one**.

Forty-seven had landed days earlier under different commit names. `--no-merged`
could not see that, because it asks about *ancestry*, and re-landing a patch
under a new message preserves the work while destroying the ancestry. So the
list said "fifty things are pending" every day until nobody read it — which is
the expensive part, because the branches that *were* live were in that list too.

The full adjudication is in
`docs/evidence/BRANCH_ADJUDICATION_2026_08_31.md`.

## What it does

For every branch not merged into the base, it asks whether the branch holds
anything the base lacks, three ways:

1. **Patch equivalence** (`git cherry`) — catches commits that landed with a
   rewritten message. This is what produced all forty-seven.
2. **Branch-unique files** — catches whole files the base has never seen. Files
   the base *deleted on purpose* are counted separately: a branch still carrying
   a deleted file is holding a corpse, and counting it as work is how you
   resurrect something the trunk decided to bury.
3. **Age** — not evidence, just the prior for how hard to look.

Clean on 1 and 2 is a **RELANDED** retirement candidate. Anything else is
**REVIEW** — the script never says "land it", because that call needs a human
reading the diff, and getting it wrong deletes work.

## Usage

```sh
tools/branch-census/census.sh
tools/branch-census/census.sh --live 'fee-core|fee-tx2|genseven2'
tools/branch-census/census.sh --base origin/main --quiet
```

`--live` takes an egrep pattern of branches known to belong to running lanes;
they are listed under their own heading rather than nagged about as REVIEW.

Exit codes follow `tools/ci/run.sh`: **0** the census ran, **2** a prerequisite
is missing. There is deliberately no exit 1 — this is a report, not a gate. A
tree with work in progress is healthy, and a check that failed CI for that would
be uninstallable.

## The stale-base trap

Run this classification in the public wrapper repo against its *local* `main`
and it reports ~1600 unlanded commits per branch, arguing to land six branches
that are every one of them already ancestors of `origin/main`. The local ref was
1712 commits stale. The classification was not wrong; the **base** was — and a
wrong base fails in the expensive direction, inventing work rather than hiding
it.

So the script picks its own base and says so: if `origin/main` exists and local
`main` is behind it, it measures against `origin/main` and prints why. If there
is no remote at all — this tree's case — local `main` is the trunk. What it will
not do is measure against a base it has not checked.

## Retiring something it flags

Write the tombstone **first**: branch name, tip SHA, what it held, and the commit
that superseded it. After `git branch -D` the tip SHA is the only way back
(`git branch <name> <sha>`), so the record has to exist before the deletion, not
after. `docs/evidence/BRANCH_ADJUDICATION_2026_08_31.md` is the format.
