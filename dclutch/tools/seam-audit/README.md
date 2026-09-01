# seam-audit

A standing gate for the six mechanical seam-defect classes that
`docs/evidence/SEAM_AUDIT_2026_08_29.md` found by hand, plus a seventh —
`AUTHORITY` — added 2026-09-01 for C-16's *unexplained authority* clause, which
no reader here answered.

`AUTHORITY` is worth distinguishing from `PRIVILEGE` because they look alike and
point opposite ways. `PRIVILEGE` reads an exact-privilege census as
**over-constraint**: privileges merge across a transaction, so a blanket signer
refusal makes a frame unsatisfiable for a legitimate builder, and the class
finds routes that refuse what they should admit. `AUTHORITY` asks the
**under-constraint** question — does the act establish who may perform it, or
read an answer somebody else supplied? `PRIVILEGE` never asks whether a signer
is the *right* signer.

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
tools/seam-audit/seam_audit.py --commit <rev>  # audit a committed tree
```

Exit codes carry a distinction the gate depends on: **1** means this tree has a
seam disagreement, **2** means the checker could not run (no `ast-grep`, an
unreadable baseline, a `--root` that is not a repository). A release gate must
not report the second as the first.

### Which tree each mode reads

The gate reads the **working tree**. You want to be told about the defect you
just wrote, before you commit it.

`--write` reads a **committed tree**, always — `--commit` if you name one,
otherwise `HEAD`. That is structural rather than advisory, because this is a
shared checkout with many concurrent authors: a `--write` that read the
filesystem would bake whatever half-finished file a neighbour had open into a
committed register, silently, since an unfinished file looks exactly like a
finished one to a static reader. The mode cannot see the working tree at all.

It exports with `git archive` rather than `git worktree add` — no repository
state is touched, so it cannot contend on `.git` locks with other lanes, and
cleanup is an `rm` rather than bookkeeping that can be left half-done. Two
seconds. Outside a git work tree `--write` refuses rather than falling back.

After writing it prints every Rust file that differs between that commit and
your working tree: those are exactly the files the register does *not* describe.
If your own fix is in one of them, commit it and rerun, or the gate will keep
reporting the finding you already fixed.

Twenty seconds over 963 Rust files. Needs Python 3.11+ and `ast-grep` on PATH
(`cargo install ast-grep`); no cargo build, no SBF toolchain, no validator, no
network. It never builds, signs, submits or contacts a cluster.

`--root` matters: `~/dev/dragons-clutch` carries a **stale squashed subtree** of
this repo in which today's fixed defects still stand. A checker pointed there
reports them as live. Gate on `~/dev/dclutch`.

## The seven classes

| class | reads | its negative control |
|---|---|---|
| `SEED_LEN` | a PDA seed domain over Solana's 32-byte maximum, and every domain with no compile-time assert holding it there | `fb076ec6` (SEAM_AUDIT #8), `fee868c5` |
| `DERIVATION` | one domain spelled two ways: differing seed arity across sites, a domain erased behind a `&[u8]` parameter, or a tuple restated outside the crate that owns it | `9a9f1b5c` (#3), `eae9a0c9` |
| `PIN_CENSUS` | a no-duplicate census over a frame whose own spec pins two coordinates equal — required to repeat and forbidden to repeat | `3b98ea3a` (#12) |
| `UNSET_PIN` | a wire pubkey used as an identity with no guard against the all-zero one; plus a ratchet on every existing guard | *synthetic* — see below |
| `DOMAIN_DUP` | two names carrying one byte string, or a name whose bytes carry none of what it claims. Matched on **bytes**, never on identifier | *live* — two unfixed collisions |
| `PRIVILEGE` | an exact-privilege census that constrains the whole transaction rather than this instruction's frame | `16351a13`, and `#13b` live |
| `AUTHORITY` | a cached role read out of an account whose provenance this function never established — no delegation to the blessed authenticator, no derived address, no owner check | *synthetic* — delete Custody's delegation and the reader must name it |

## Negative controls

```
tools/seam-audit/negative-controls.sh          # all 11, ~4 min
tools/seam-audit/negative-controls.sh SEED_LEN # one class
```

**A checker that has never caught a known defect is decoration.** Every reader
is held to one of four bars, and which one it gets is decided by what the tree
can supply rather than by what would look tidiest:

- **historical** — check out the fixing commit's *parent* into a throwaway
  worktree and require the reader to find the defect there **and** to be silent
  at HEAD. Silence after is half the bar: a reader that also fires on the fix is
  reading the code around the defect rather than the defect.
- **live** — the defect is documented, unfixed, and in the tree now. Require the
  reader to name it at HEAD, at the right function. This is a *stronger* bar,
  not a weaker one.
- **silent** — the mirror of `live`, and it earns its own kind for the same
  reason: a reader that fires on code which has closed the hole *in place* is
  not stricter, it is wrong, and a class nobody trusts gets switched off. Both
  of the sites under this bar were reported by the reader these controls
  replaced, so each fails against it.
- **synthetic** — `UNSET_PIN` has no 2026-08-29 defect behind it (the charter
  said the class was "swept clean today"; it was not — the audit records no such
  finding and no commit that day touches the pattern). So the control mutates a
  worktree to delete a live guard and requires the ratchet to notice. That class
  says so in its own docstring rather than borrowing credibility from the
  others.

Worktrees only. Never `git stash` — this is a shared tree.

Unit tests for the reading machinery underneath: `tools/seam-audit/test-seam-audit.py`
(32 cases, all of them things the checker got wrong at some point while it was
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

At `fd8cad39` the register holds 648 entries, of which **4 are confirmed open
defects** — SEAM_AUDIT #13b (found by this checker unaided) and the Claims
seed-domain byte collision that makes a V4 and a V5 constant derive the same
address. It was five at `05372c0f`: the `PROTOCOL_POSITION` /
`RATIONAL_CLAIMS_CUSTODY` collision was fixed without the register being told,
and the `GONE` half of the ratchet is what noticed.

From `fd8cad39` the register also records the revision it was measured at, in
`measured_commit`. What the baseline pins is still the finding *set* — the gate
is green wherever main is, so long as the set reproduces — but a set with no
revision attached cannot be reproduced by anyone later, or shown to describe
committed code rather than whatever happened to be on disk.

## Wiring it into a gate

One command, exit nonzero on new findings:

```sh
python3 tools/seam-audit/seam_audit.py
```

It needs nothing but a checkout, so it belongs early — before any build, where
it costs twenty seconds and can refuse a seam disagreement before an SBF
toolchain is spun up.

Still **not** wired into `tools/release/`, and still for the same reason: that
script is owned by whoever is cutting the live cohort, and changing what the
release gate accepts while a revision-pinned devnet cycle is mid-flight moves
the target. The line below is written and tested against a real export; it is
the cutting lane's to land.

Insert it after the `artifact_provenance.py` `cmp` block in
`checked-release-candidate.sh`, ahead of the toolchain section:

```sh
python3 "$SOURCE/tools/seam-audit/seam_audit.py" \
    || { echo "refusing: seam-audit gate reports drift against its register" >&2; exit 1; }
```

Run the **archived** copy, not `$SCRIPT_DIR`'s. Unlike its three neighbours it
then needs no `cmp` pin: its `--root` and `--baseline` defaults both resolve
inside `$SOURCE` from `__file__`, so it is bound to `--commit` by construction
rather than by a check that could be forgotten. Verified green in a plain
`git archive` export with no `.git` in it.

One prerequisite, and it is not optional: **the release host needs `ast-grep` on
PATH.** The checker is pattern-driven and has no fallback. Without it the gate
exits 2 with a message saying so — deliberately not 1, which would report "the
checker could not start" as "this tree has a seam defect".

For CI, the same one line. The gate is deterministic and reads no state outside
the checkout, so it needs no ordering against anything else.
