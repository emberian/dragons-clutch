# tools/sbom — dependency/license closure

`SBOM.md` in this directory is generated, not hand-written: every dependency
this repository actually resolves, with its license, source, and how that
license was determined, across every tracked Cargo workspace and npm package
tree discovered from the repository manifests. The generated Coverage section
is the authority for the current set; no separate workspace count is maintained
here.

```sh
tools/sbom/sbom_check.py            # regenerate SBOM.md
tools/sbom/sbom_check.py --verify   # check for drift; writes nothing; exit 1 on drift or a classification failure
python3 -m unittest tools/sbom/test_sbom_check  # offline classification-logic tests
```

## What runs this automatically

Two things, and both are in this repository:

- **`tools/gate sbom`** — the `sbom` tier. It runs the classification
  tests first and `--verify` second, so "the checker is broken" and "this tree
  has a licence defect" arrive as different answers. It needs `cargo` (the
  closure resolves every tracked workspace with `cargo metadata --locked
  --offline`) and reports a missing one as exit 2, never as a pass. It is in
  the `all` alias and, at about three minutes, deliberately not in `cheap`.
- **`apps/dclutch-web`'s `npm test`**, via `lib/sbomVerify.test.ts`.

`tools/gate web` excludes that vitest case by name, because it
needs a populated cargo registry that a Node-only job does not have. The `sbom`
tier is where the assertion lives instead, with that prerequisite declared.

Nothing in `tools/gauntlet` runs this. An earlier version of the generated
`SBOM.md` header said otherwise; it was wrong, and it was wrong in a generated
file, so it reprinted itself on every regeneration.

## Why this exists

GITSCAN-2's G-4 (`docs/ASPIRATION_LEDGER.md`): gen-1 (`dragons-clutch`,
`scripts/dependency_license_check.py`) ran a real dependency/license closure
— 36 manifests, 2,129 rows, PASS, a committed catalog byte-gated against
drift — and left three dependency families for a human to review. Gen-3 (this
repository) had no such instrument at all, on a strictly larger surface (an
npm tree gen-1 never had), while the Pages workflow now distributes the
frontend, which makes this repository's `AGPL-3.0-or-later` source-offer
obligation live rather than theoretical. This tool is gen-1's method,
generalized to this repository's actual shape — see `sbom_check.py`'s module
docstring for the full classification and flagging rules.

## What it checks

- Every tracked `[workspace]`-bearing `Cargo.toml`, resolved with
  `cargo metadata --locked --offline` against its own committed `Cargo.lock`.
  A tracked `Cargo.toml` without `[workspace]` is a *member*, adopted by the
  nearest ancestor workspace regardless of whether it is literally named in
  that ancestor's `members` (Cargo's own directory-walk rule) — checking
  members individually would recompute the same closure repeatedly and
  misattribute its rows, so only discovered workspace roots are queried
  directly.
- Every tracked `package.json`; dependency-bearing package trees resolve from
  their adjacent `package-lock.json`. New package trees are picked up
  automatically.

License expressions are evaluated by their SPDX operators rather than matched
as substrings: `AND` conjoins obligations (every arm must be permissive),
`OR` is a choice granted to the licensee (one fully permissive arm is
enough). So `MIT OR Apache-2.0 OR LGPL-2.1-or-later` is not a review item —
a permissive arm is on the table and no copyleft obligation can be compelled
— while `Apache-2.0 AND LGPL-3.0-or-later` still is. Rows cleared this way
are listed in `SBOM.md` anyway, under *Cleared by the permissive-arm rule*,
so a reader who sees `GPL` in a license column can find why it was not a
question. An expression the evaluator cannot parse is never cleared.

Four things it reports without failing the gate over them (each gets its
own section in `SBOM.md`, so none of this is swept away):

- **Flagged for review** — a license this tool cannot mechanically clear:
  anything copyleft or copyleft-adjacent on a *third-party* dependency
  (AGPL/GPL/LGPL/SSPL/MPL/CDLA/CDDL/EPL/OSL/EUPL edges), any
  `LicenseRef-file:` row (a license file, digest-pinned, never guessed into
  an SPDX id), and anything else not on the small, explicit permissive
  allowlist. This is the deliverable for ember's counsel item.

- **Reviewed and allowed** — the flags that have been *answered*. Closing a
  review item is a human act and the tool records it as one: each entry in
  `REVIEWED_ALLOWANCES` (in `sbom_check.py`) names what it covers, the reason
  it was decided that way, and the evidence it rested on, and the generated
  section lists the rows under each ruling. A ruling moves a row out of the
  open queue into the visible record — it never deletes one, and it never
  touches `NOTICES.md`, because reviewing an attribution obligation does not
  discharge it. Every entry is pinned to the exact license expression that
  was read (for a `LicenseRef-file:` row, to the file's `sha256`), so a
  dependency that changes license on an upgrade stops matching and returns to
  the queue by itself. An entry that stops covering any row is reported as
  stale rather than left to accumulate.

  Add one when a license question has actually been decided — never to quiet
  a row you have not read. `--verify` byte-gates the result, so an allowance
  that changes what the report says cannot land without the regenerated
  `SBOM.md` beside it.
- **Unresolvable manifests** — a mini-workspace whose `Cargo.lock` does not
  match its `Cargo.toml` (a dependency edge changed without re-running cargo
  there). A reproducibility gap, not a license question; owed to whichever
  lane owns that manifest.
- **Stray lockfiles** — a `Cargo.lock` sitting next to a workspace *member*
  manifest. Cargo never reads it (it resolves through the owning workspace
  instead), so it is dead weight and ordinary repository hygiene, not an SBOM
  concern; left in place, not this tool's file to delete.

What *does* fail `--verify`: a genuinely unclassified license (no SPDX
expression, no `license_file`, and — for a first-party crate — not
`publish = false` either, so there is nothing to fall back to), a forbidden
dependency source (git, a path outside this repository, an unrecognized
registry), and any drift between a fresh run and the committed `SBOM.md`.

## The notice page

`tools/genref/render-site.mjs` renders `SBOM.md`'s "Notices" section into the
Pages artifact (`notices.html`, linked from the landing page) — the mechanical
aggregation the AGPL source-offer obligation wants once the site actually
distributes the frontend, not authored legal text.
