# tools/sbom — dependency/license closure

`SBOM.md` in this directory is generated, not hand-written: every dependency
this repository actually resolves, with its license, source, and how that
license was determined, across the whole tree (there is no single Cargo
workspace — 38 independent ones — plus the web app's npm tree).

```sh
tools/sbom/sbom_check.py            # regenerate SBOM.md
tools/sbom/sbom_check.py --verify   # check for drift; writes nothing; exit 1 on drift or a classification failure
python3 -m unittest tools/sbom/test_sbom_check  # offline classification-logic tests
```

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

- Every `[workspace]`-bearing `Cargo.toml` (the repository root plus 37
  self-contained test-program/tool mini-workspaces), resolved with
  `cargo metadata --locked --offline` against its own committed `Cargo.lock`.
  A tracked `Cargo.toml` without `[workspace]` is a *member*, adopted by the
  nearest ancestor workspace regardless of whether it is literally named in
  that ancestor's `members` (Cargo's own directory-walk rule) — checking
  members individually would recompute the same closure repeatedly and
  misattribute its rows, so only the 38 workspace roots are queried directly.
- Every tracked `package.json` with its `package-lock.json` (today:
  `apps/dclutch-web`; new ones are picked up automatically).

Three things it reports without failing the gate over them (each gets its
own section in `SBOM.md`, so none of this is swept away):

- **Flagged for review** — a license this tool cannot mechanically clear:
  anything copyleft or copyleft-adjacent on a *third-party* dependency
  (AGPL/GPL/LGPL/SSPL/MPL/CDLA/CDDL/EPL/OSL/EUPL edges), any
  `LicenseRef-file:` row (a license file, digest-pinned, never guessed into
  an SPDX id), and anything else not on the small, explicit permissive
  allowlist. This is the deliverable for ember's counsel item.
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
