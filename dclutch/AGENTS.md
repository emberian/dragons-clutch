# dClutch agent instructions

dClutch is a greenfield Solana protocol for fully collateralized bounded-state
claims. `~/dev/dragons-clutch` is compost: study it for requirements,
invariants, counterexamples and measurements; never copy it wholesale. These
are the rules a lane needs. The incidents that taught each of them are in
`docs/ledger/AGENTS_2026-09-03.md` (the previous text of this file, verbatim)
and the dated ledger it links to.

## Authority

- Default work is offline. Never read wallet or private-key dotfiles or
  browser sessions; the devnet keypair this work requires is the exception.
- Never sign, submit, deploy, fund, publish, push, tag or mutate an external
  system without explicit current authorization naming that act.
- **Devnet deploy is authorized, standing (ember, 2026-09-01)**, on three
  conditions: a full redeploy of every program in the cohort from exact
  current sources with fresh identities, the old cohort abandoned in place;
  the load simulator runs against the new cohort as part of the deliverable;
  and the deploy is from a commit, never the dirty tree. Record the commit,
  every ELF hash, every program id, every signature and the poststates. The
  runbook is `tools/cohort/`.
- **Publication cuts are authorized, standing (ember, 2026-09-01)**, only
  through `tools/cut.sh`: it cuts from HEAD, runs the credential sweep as a
  value test that must find zero, and refuses unless the published `dclutch/`
  tree object is identical to HEAD's. Cut whenever a batch of lane work lands.
- **Not authorized**: mainnet anything, tags, releases, force-pushes, pushing
  any branch other than the cut to `main`.
- Public RPC reads need an explicit task and stay bounded. Never call a
  deployment or frontend official without a checked release manifest.

## The tree, the shell, and commits

- **The live tree is `/Users/ember/dev/dclutch`.** `~/dev/dragons-clutch/dclutch`
  is a frozen publication copy, and the shell's working directory resets
  between commands. Use absolute paths, and print `git rev-parse --show-toplevel`
  and `git rev-parse HEAD` in the same command as any measurement you report.
- **Export `DCLUTCH_LANE=<lane name>` before your first commit**, and commit
  only through `tools/lane.sh commit <msg> -- <paths>` (or `commit-patch` when
  two lanes edit one file; it commits your hunk alone and carries it into the
  working tree). Read `git diff` on your own paths after either. Every lane
  commits as the same author; the `Lane:` trailer is the only attribution.
- The dirty tree is deliberate: never `stash`, `reset`, `clean` or `add -A`;
  never delete under `~/dev`. Inspect the complete staged path list before
  every commit; a named-file `add` does not make a whole-index commit safe.
- **A commit carries the manifest and lock its code needs.** Before
  committing ask what the change depends on that HEAD does not have;
  `cargo metadata --locked` answers it in seconds.
- **Never delete `.git/index.lock`**: 1Password's `op-ssh-sign` holds it while
  another lane's commit waits on a prompt. Check `ps` for a live
  `git`/`op-ssh-sign` and wait. An unsigned commit of your own paths is
  allowed when signing is what is stuck.
- **Backticks in a double-quoted shell message are command-substituted**, in
  `git commit -m` and in `lane.sh board` alike. Write the message to a file
  and use `-F`, then read back what landed.
- Never point a nested program-test workspace at the root `CARGO_TARGET_DIR`;
  each is its own workspace with its own `target/`. `cargo metadata` is the
  discriminator when a link fails with "multiple different versions".
- Run `cargo check` on the touched workspace before any SBF build. Heavy
  Linux builds go to hbox through `swarm-build`, never bare `taskset`.

## Provenance

- Do not import, copy or depend on code from JOSHI, joshibot, leanuweave,
  minidregg, breadstuffs, Oracle Pit or the historical DREGG prototypes.
- Dragon's Clutch code is transplanted only through `COMPOST.md`: name the
  invariant, source commit and path, licence, new semantic owner and
  adversarial tests. Prefer a fresh implementation from the written invariant.
- Never merge an old implementation to preserve sunk work.

## Architecture

- Keep the universal Market Core small. Venues, liquidity, wrappers, bearer
  mints and recovery depth are capability children, not universal ontology.
- Persist economic facts and replay authority, not an offchain workflow graph.
- One semantic owner per persisted fact; a separate concept does not
  automatically deserve a separate account.
- Market capabilities are immutable and canonically identified. Deferred
  physical creation is precommitted and prepaid.
- Hoard principal is never fees, rent, bounty, insurance, work funding,
  reserve or treasury capital. Future revenue is never present capitalization.
- Static clients, routers, matchers and indexes are untrusted projections.
- Do not preserve parallel legacy/current authority paths: when a successor
  is accepted, delete the superseded path in the same convergence cycle.
- **Banishing a program, crate or route is finished only when every non-Rust
  consumer is swept in the same commit** — `apps/dclutch-web` above all, since
  a browser that mirrors a wire by hand becomes its last authority the moment
  its owner is deleted. `npm run abi:coverage` lists what the browser states
  in its own words; every generated module carries an `abi:*:verify`.

## Correctness vocabulary

- A state partition must be exhaustive, disjoint, ordered and canonical
  before it can mint liabilities.
- Portfolio payoffs and simplex prices use exact scaled integers with one
  named rounding boundary.
- Say "best valid submitted candidate", never "optimal clearing", without a
  checked optimality certificate.
- Fixtures, simulation, local-validator execution and devnet execution are
  distinct evidence levels; none is mainnet evidence.
- Never call the protocol formally verified without naming the theorem,
  source digest, toolchain, assumptions and unverified runtime boundary.
- Every fixed bound is labelled mathematical, chain-derived, measured-profile
  or provisional; a provisional bound carries a lifting plan
  (`docs/design/CLIFF_DOCTRINE_V1.md`).

## Refusal codes

Every program error code is namespaced by program (decision 0007;
`crates/dclutch-refusal-registry` is the authority): `band = code >> 12`,
band 0 is never allocated, bands are append-only and a deleted program's band
is withdrawn, never reused.

- A refusal enum that can reach the chain carries `#[repr(u32)]` and explicit
  hexadecimal discriminants inside its band, pinned to the registered base by
  a `const _: () = assert!(...)`. `#[repr]` on an `*Error` enum is the
  declaration "protocol-visible"; the census enumerates nothing else.
- Never write a refusal code as a bare number — not in a test, a binding or a
  doc comment. Derive it from the enum or the registry base.
  `assert!(text.contains("Custom(3)"))` also accepts `Custom(30)`.
- **A bare `is_err()` is a test of nothing**: it passes on whatever the
  transaction refuses first. Name the exact discriminant and prove the test
  red before trusting it green. If the refusal you want to name is one code
  over many conjuncts, split the discriminant; do not rewrite the test.
- **`map_err(|_| Coarse)` converts a located defect into a search.** Prefer
  `map_err(Specific::from)` whenever the callee distinguishes causes; if the
  wire cannot carry the distinction, keep the cause in a `msg!` on the
  refusing path. A coarse code is legitimate only when the causes are one
  accusation. To localize one anyway: the tree's own instrument first
  (`hot-cu-profile`'s checkpoints), then markers, then the public entry point
  over longer prefixes. "Could not be localized" is an output.
- `tools/gate census` is the gate (`dclutch-route-census inventory --check-unique`),
  and it runs in the `cheap` tier.

## Generated artifacts

- **`tools/gate reference` refuses a dirty tree (exit 2)** because it reads
  the working tree. Regenerate from a detached worktree at HEAD, and run
  `--converge`, never the generator twice: the reference and the client
  mirrors close a cycle, and `--converge` settles it or refuses.
  `tools/gate reference --check --converge` asks the same of a committed revision. The
  convergence owner runs it; lanes do not. `docs/reference/` is generated and
  never hand-edited.
- Never redirect a generator into a canonical tracked output. Emit to a
  temporary file on the same filesystem, require exit zero, validate the
  header and width, then replace atomically; a failed generator leaves the
  last accepted output byte-for-byte intact.
- **A commit that changes any crate compiled into an SBF link carries its
  frame baseline rows (`tools/gates/frames-baseline.json`) or says in its
  message that it leaves the ratchet red** — the link is its whole
  path-dependency closure. Capture with `tools/gate frames --at <commit>
  --capture <file>` in the same commit; `tools/gate frames owed` names who has
  not.

## Kernel policy

The first-party kernel is `no_std`, `no_alloc`, safe Rust, fixed-layout and
total: no `unsafe`, FFI, floating point, dynamic allocation, unchecked casts,
proof-only preconditions or hidden executable alternatives. Public functions
validate untrusted input and return explicit errors. Solana SDK, Token-2022,
oracle SDKs, CPI, account memory, RPC and transaction construction live
outside the kernel in named adapters.

## Measurement

- **Distrust silent success.** A command that reports success having done
  nothing makes every downstream check measure an absence: `swarm-build` can
  exit 0 having executed nothing (`Unit run-u<N>.scope was already loaded` —
  grep for it and retry); a `cp -a` cut by a timeout leaves a partial
  directory a `test -d` guard then skips (use `rsync -a --delete`); a suite
  runner under `set -e` stops at its first failure and reports one number.
  Run every row, report every row, keep "failed" distinct from "never ran".
- **Program logs from one test binary interleave.** Re-run with a name filter
  and `--test-threads=1` before believing which test refused, or any width,
  ordering or count read from interleaved output.
- **A probe measures what it touches, not what you meant.** Verify the
  instrument before believing the reading; a `timeout … | head` reports
  head's exit, so capture the status inside the subshell.
- Never run an unfiltered `-p <crate>` suite, locally or on hbox. Filter to
  what could refute you and state your control separately.
- Measure, refute the obvious suspect, and only then move a constant or
  convict code. A named wall is a deliverable; a green fixture proving the
  wrong thing is not.

## Project conduct

- One canonical integration branch. Lanes are short-lived and bounded, touch
  disjoint files or coordinate first, and use `tools/lane.sh` for commits,
  pinned `rustfmt`, board entries and running a script another lane might
  edit (`tools/lane/README.md`).
- Build vertical executable slices: kernel semantics, adapter, operator
  construction and an honest user-visible status; no layer claims completion
  alone.
- Do not substitute repeated audits for implementation; a review ends in a
  decision, a deletion, a test or a queued owner.
- Add adversarial tests with each invariant or parser. Never weaken a refusal
  to make an integration test pass.
- Every fact has one author: a decision lives in its record
  (`docs/decisions/`), a cohort fact in its evidence document, a design fact
  in its note's head, a number in the generated reference, and a dated delta
  is one row in `GOAL.md` linking to the store. Evidence documents are dated
  and never edited; a reversed verdict gets an in-place, dated addendum.
