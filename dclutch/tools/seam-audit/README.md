# seam-audit

A standing gate for the six mechanical seam-defect classes that
`docs/evidence/SEAM_AUDIT_2026_08_29.md` found by hand.

That sweep found nine always-refuses routes and one always-admits across six
seams, **none of which had a failing test**, and its closing paragraph says why:

> a green suite is evidence about fixtures, not about seams

Each side of every one of those seams was tested against a fixture that same
side authored, so both sides were green and the composition was dead. The
Token-2022 writer and its readers each fabricated the mint bytes they expected;
Trading's record test derived with the checker's own wrong spelling; the
capability activation test ran against a Core *stub* strictly more permissive
than Core in exactly the dimension that broke. No additional test fixes that,
because the defect is the absence of a *joint* author.

This checker is the joint author. It reads both sides statically and refuses
the disagreements, so the class dies as a category instead of being re-hunted by
hand.

## Running it

```
tools/seam-audit/seam_audit.py                 # the gate; exit 1 on drift
tools/seam-audit/seam_audit.py --report        # every finding, with reasons
tools/seam-audit/seam_audit.py --write         # retriage into the baseline
tools/seam-audit/seam_audit.py --class SEED_LEN --report
tools/seam-audit/seam_audit.py --root <dir>    # audit another checkout
```

Twenty seconds over 963 Rust files. Needs Python 3.11+ and `ast-grep` on PATH
(`cargo install ast-grep`); no cargo build, no SBF toolchain, no validator, no
network. It never builds, signs, submits or contacts a cluster.

`--root` matters: `~/dev/dragons-clutch` carries a **stale squashed subtree** of
this repo in which today's fixed defects still stand. A checker pointed there
reports them as live. Gate on `~/dev/dclutch`.

## The six classes

| class | reads | its negative control |
|---|---|---|
| `SEED_LEN` | a PDA seed domain over Solana's 32-byte maximum, and every domain with no compile-time assert holding it there | `fb076ec6` (SEAM_AUDIT #8), `fee868c5` |
| `DERIVATION` | one domain spelled two ways: differing seed arity across sites, a domain erased behind a `&[u8]` parameter, or a tuple restated outside the crate that owns it | `9a9f1b5c` (#3), `eae9a0c9` |
| `PIN_CENSUS` | a no-duplicate census over a frame whose own spec pins two coordinates equal — required to repeat and forbidden to repeat | `3b98ea3a` (#12) |
| `UNSET_PIN` | a wire pubkey used as an identity with no guard against the all-zero one; plus a ratchet on every existing guard | *synthetic* — see below |
| `DOMAIN_DUP` | two names carrying one byte string, or a name whose bytes carry none of what it claims. Matched on **bytes**, never on identifier | *live* — two unfixed collisions |
| `PRIVILEGE` | an exact-privilege census that constrains the whole transaction rather than this instruction's frame | `16351a13`, and `#13b` live |

## Negative controls

```
tools/seam-audit/negative-controls.sh          # all of them, ~4 min
tools/seam-audit/negative-controls.sh SEED_LEN # one class
```

**A checker that has never caught a known defect is decoration.** Every reader
is held to one of three bars, and which one it gets is decided by what the tree
can supply rather than by what would look tidiest:

- **historical** — check out the fixing commit's *parent* into a throwaway
  worktree and require the reader to find the defect there **and** to be silent
  at HEAD. Silence after is half the bar: a reader that also fires on the fix is
  reading the code around the defect rather than the defect.
- **live** — the defect is documented, unfixed, and in the tree now. Require the
  reader to name it at HEAD, at the right function. This is a *stronger* bar,
  not a weaker one.
- **synthetic** — `UNSET_PIN` has no 2026-08-29 defect behind it (the charter
  said the class was "swept clean today"; it was not — the audit records no such
  finding and no commit that day touches the pattern). So the control mutates a
  worktree to delete a live guard and requires the ratchet to notice. That class
  says so in its own docstring rather than borrowing credibility from the
  others.

Worktrees only. Never `git stash` — this is a shared tree.

Unit tests for the reading machinery underneath: `tools/seam-audit/test-seam-audit.py`
(25 cases, all of them things the checker got wrong at some point while it was
being written).

## The register

`baseline.json` is the triaged register; `EXCEPTIONS.md` gives the reason for
every verdict tag in it. The gate refuses a tag with no written reason, so an
exception cannot be accepted by editing JSON alone.

The ratchet turns **both ways**, following
`packages/dclutch-sdk/scripts/abi-coverage.mjs`:

- a finding not in the baseline fails as `NEW`;
- a baseline entry that no longer reproduces fails as `GONE`, because a defect
  that was fixed must *leave* the register rather than stand as cover for the
  next one. Rerun `--write` when you fix something.

A verdict is a claim, and the tags do not blur into each other. `benign-*` says
it is not a defect. `debt-*` and `hazard-*` say it **is** one, of a known shape,
not fixed in this lane. `inventory-*` is not a finding. The gate prints the
verdict census on every run for exactly this reason: a register whose entries
all read "accepted" looks identical to a clean tree, which is the failure the
audit was about.

At `05372c0f` the register holds 634 entries, of which **5 are confirmed open
defects** — SEAM_AUDIT #13b (found by this checker unaided) and two seed-domain
byte collisions in the Claims family, one of which makes a V4 and a V5 constant
derive the same address.

## Wiring it into a gate

One command, exit nonzero on new findings:

```sh
python3 tools/seam-audit/seam_audit.py
```

It needs nothing but a checkout, so it belongs early — before any build, where
it costs twenty seconds and can refuse a seam disagreement before an SBF
toolchain is spun up.

Deliberately **not** wired into `tools/release/` in this lane. The checked-release
gate's semantics were live during the cohort freeze on 2026-08-29 and changing
what that gate accepts while a revision-pinned devnet cycle is mid-flight would
move the target. The additive call is one line wherever the release script runs
its other local-evidence checks, and it should be added by whoever owns that
script once the cohort lands.

For CI, the same one line. The gate is deterministic and reads no state outside
the checkout, so it needs no ordering against anything else.
