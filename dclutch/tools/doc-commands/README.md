# doc-commands — a runbook nobody replays is a runbook nobody can trust

A runbook is an instruction to a reader **now**. Nothing in this tree checked
that its instructions still work. A flag gets renamed, a script grows a required
argument, a binary is renamed — and the sentence telling a stranger to type it
stays exactly where it was, keeping all of its authority. **Nothing goes red.**
The cost is paid by the one person least able to absorb it: someone who does not
already know what the command was supposed to do.

This is the doc half of [`tools/doc-citations`](../doc-citations/README.md),
which closed the same shape for symbols named in doc comments. There, a citation
outlived the thing cited. Here, an instruction outlives the interface it
instructs. It is also the outer half of the release tier's `usage_parity.py`,
which holds a *tool's* usage text to its own parser; this holds a *runbook* to
the program it instructs, and neither can see the other's half.

```sh
python3 tools/doc-commands/doc_commands.py --root .
python3 tools/doc-commands/doc_commands.py --root . \
    --baseline tools/doc-commands/baseline.json --check   # the gate
sh tools/doc-commands/negative-control.sh                 # both directions
tools/ci/run.sh runbooks
```

## What it found the day it was written

Four defects, all live at `HEAD`, none visible to any existing gate:

- **`README.md:166` and `docs/guides/reader.md:67`** published
  `tools/release/private-validator-lifecycle/run.py --through participant` under
  the heading *"See it run"*. That program requires `--repo`, `--release-root`,
  `--validator`, `--solana` and `--work`. **The single most prominent "try it"
  command in the repository could not be run by anybody**, and the way to notice
  was to run it.
- **`dclutch-terminal --help` did not admit its own flags existed.**
  `docs/guides/trencher.md` teaches `intent buy --route --outcome --fill --price
  --collateral`; not one of those five appeared in `--help`, because the help
  text was hand-written prose beside a `FLAG_OPTIONS` table it had drifted from.
  Repaired at the source: the help now RENDERS that table, so a flag added to the
  parser appears in the help in the same edit or not at all.
- **`tools/release/devnet-price-update.sh --help` printed lines 2–10 of itself**,
  a hardcoded range that stopped two lines above the flags the guide passes it.
  It now finds the end of its own comment block instead of counting to it.
- **`tools/ticket-board/run-local.sh` was tracked without its execute bit**, so
  the command `docs/operators/author-a-ticket.md` publishes could not start.

And then it found two in the page written to answer it, which is the argument
for having it: `docs/guides/two-clients.md` published `dclutch-terminal markets
ls | markets show <address>`, using `|` as prose alternation inside a `sh`
fence, where a reader's shell reads a pipe.

## Scope, stated rather than assumed

`README.md`, `docs/guides/`, `docs/operators/` — the documents a reader is
*sent to*. `docs/evidence/` records what a past run did and its commands are
dated by construction; holding a record to today's interface would be holding
the wrong thing to the wrong standard. `--roots` takes the scope explicitly, so
widening it is a decision somebody makes on purpose.

## The two tiers, because they fail differently

**RESOLVED** — the program the command names exists. A repo-relative path is
tracked and executable; a bare name is a bin this repository declares. The
declared-bin list is read from the npm `bin` maps and cargo `[[bin]]` names, not
kept here: a list of binary names inside a checker is a second authority for a
fact the manifests own, and it is wrong the day somebody renames one — which is
exactly what just happened to `dclutch`.

**PROBED** — the program accepts the subcommand and the long flags the runbook
passes it, established by running `<program> --help` and reading the output.
Required arguments come from the program's own `usage:` line: argparse prints
optional things in `[...]` and required things bare, so stripping the bracketed
spans leaves exactly what a reader must pass.

The probe **descends into a subcommand's own help page**, through words the
current page already names, and judges a flag against every page on that path —
because a reader walks the same path. A CLI that documents its subcommands
there is healthy, and demanding every flag on the top-level page would report a
defect where there is none: `dclutch ticket author` takes fourteen flags that
`dclutch ticket --help` names and `dclutch --help` rightly does not.

A declared binary is looked for **beside its own manifest first**, then in the
root `target/`. That is not a nicety: `tools/dclutch-cli` declares a
`[workspace]` of its own, so `dclutch` builds to
`tools/dclutch-cli/target/release/`, and a checker that only looked at the root
one called the repository's published binary unbuilt while it sat there
compiled.

## What it will not do

It runs `--help` and nothing else. It never runs a command a runbook publishes,
never touches a chain, and probes **only** a program whose own source shows it
*handles* a help flag — safety by declaration, not by hope.

That distinction is not pedantic and was paid for while this was being written:
`tools/ticket-board/run-local.sh` mentions `--help` in a comment and passes every
argument through to a binary it `cargo build`s first. A checker that trusted the
mention spent sixty seconds compiling. The probe now looks for a handled help
arm, runs in its own session so a timeout kills whatever it started, and gives
up after thirty seconds.

A program it cannot probe is reported **unprobed, with the reason**, never as
passing, and the gate exits **2** for it. "Could not be checked" and "checked and
fine" are different answers.

## Exit codes

The tree's own (`tools/ci/run.sh`, `tools/seam-audit`):

| code | meaning |
| ---: | --- |
| 0 | every command resolved, and every probe that ran agreed |
| 1 | a runbook publishes something a reader cannot run as written |
| 2 | a prerequisite is missing; nothing was proven either way |

## The baseline

`baseline.json` holds *accepted defect findings* and is empty, which is the
state to keep it in. Unprobed commands are deliberately **not** baselined: what
is built in a checkout is a fact about that checkout, not about the runbook, and
freezing it would turn a prerequisite into a permanent excuse.

The fix for a finding is the **doc** or the **program**. An entry added here has
to carry a reason a reader would agree with.
