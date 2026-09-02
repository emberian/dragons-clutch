# Frameguard

`run.sh` freshly builds all 12 program links with LLVM stack-size sections and
compares every canonical function frame with `baseline.json`. It catches frame
growth below SBPF v0's 4,096-byte hard wall; the ordinary build diagnostic does
not.

```sh
tools/ci/run.sh frameguard
tools/ci/run.sh --commit HEAD frameguard  # quoteable, archived source
```

Exit 0 is agreement, 1 is a build/diagnostic/frame disagreement, and 2 means a
prerequisite or measurement was absent. The build must freshly compile each top
package and emit zero stack-overwrite diagnostics before comparison.

## A baseline names the commit it measured

The baseline is an exact ratchet, not a ceiling. Shrinkage is red until the
smaller manifest is admitted, so recovered headroom cannot be spent again. That
exactness has a cost that was paid three times on 2026-09-02: the double build
takes about four minutes, this tree takes a program commit about that often,
and so three correct recaptures were each invalidated before their author could
read the diff. The last would have admitted 26 changed rows of which 2 were its
own.

**An exact ratchet cannot be recaptured after the fact by a bystander.** So a
capture names its base: `--at <commit>` measures a detached worktree at that
commit and records it in the manifest, a capture from a dirty tree is refused,
and `accept` refuses two captures that name different commits. The diff a
reviewer reads is then between two named commits.

```sh
tools/frameguard/run.sh --at HEAD --capture /tmp/frame-a.json
tools/frameguard/run.sh --at HEAD --capture /tmp/frame-b.json   # same commit
tools/frameguard/frameguard.py accept \
  --first /tmp/frame-a.json --second /tmp/frame-b.json \
  --output tools/frameguard/baseline.json
```

The instrument is the tool you invoked, not the tool at the measured commit:
`--source`/`--at` choose only which program sources are compiled, and the
checker and frame parser come from this script's own tree unless `--tools DIR`
says otherwise. Otherwise no past commit could be measured at all -- the first
`--at` capture built all twelve links and died at the assembler, because the
commit it measured predated the checker's `--commit` flag by one commit.
(`tools/ci/run.sh --commit <rev>` archives a whole tree and measures it with
its own tools, which is a different and equally valid thing; there this script
IS the archived one.)

Read the complete baseline diff before committing it. A new function, removed
function, changed instance count, growth, and shrinkage all require that
review, and each changed row belongs to some commit in the range. Growth toward
the 4,096 bound that no commit message explains is a finding, not a row to
admit quietly.

## Who owes rows

The rule that follows is that **a commit touching `programs/*/src/**` either
carries its baseline rows or says it leaves the ratchet red** -- the recapture
has to ride with the commit that moves the frame. `owed` is that rule as a
tool rather than as prose: it reads the range back from the baseline's own
recorded commit and names the commits that did neither.

```sh
tools/frameguard/frameguard.py owed --repo . --baseline tools/frameguard/baseline.json
tools/frameguard/frameguard.py owed --repo . --since <commit>   # explicit range
```

Exit 0 is nobody owing, 1 is a named ledger, 2 is a range that could not be
read (a baseline captured before this field existed names no commit). `run.sh`
runs it automatically when the frame comparison goes red, so the CI tier
reports who owes rather than only that the frames disagree.

It is a **lower bound on authorship**: a `crates/` change reaches the links too
and is not named here. An unattributed changed row is a finding to report, not
evidence of a phantom.
