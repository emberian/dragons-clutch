# dClutch agent instructions

dClutch is a greenfield Solana protocol for fully collateralized bounded-state
claims. The neighboring `~/dev/dragons-clutch` repository is compost: it may be
studied for requirements, invariants, counterexamples, and measurements, but it
is not a source tree to copy wholesale.

## Authority and safety

- Default work is offline. Never read wallet/private-key dotfiles or browser
  sessions.
- Never sign, submit, deploy, fund, publish, push, tag, or mutate an external
  system without explicit current authorization naming that act.
- **DEVNET DEPLOY IS AUTHORIZED, standing, from 2026-09-01.** Ember: *"don't
  defer to me, whenever and as often as you feel ready, please deploy to devnet
  — just ensure that you do a full redeploy, including the load simulator."*
  This is a durable grant, not a one-time approval: deploy devnet whenever the
  tree is ready, and again whenever it is ready again. It carries three
  conditions. **(a) Full redeploy only** — every program in the cohort from
  exact current sources, fresh identities, the old cohort abandoned in place;
  no partial or incremental program deploys. **(b) The load simulator runs
  against the new cohort** and its population life is part of the deliverable,
  not a follow-up. **(c) Deploy from a commit, never from the ambient dirty
  tree** — a deployment whose sources cannot be named is unreproducible
  evidence, which C-14 forbids and which no amount of devnet success repairs.
  Record the commit, every ELF hash, every program id, every transaction
  signature and the resulting poststates.
- **PUBLICATION CUTS ARE AUTHORIZED, standing, from 2026-09-01.** Ember:
  *"mind cutting fresh to ~/dev/dragons-clutch anyway, and keeping that fresh?"*
  Run `tools/cut.sh`, which is the only sanctioned way to do it: it cuts from
  **HEAD, never the working tree** (the tree is dirty on purpose, and
  publishing a state no commit names is unreproducible evidence), runs a
  credential sweep as a **value test** that must find zero, and gates on the
  published `dclutch/` tree object being **identical** to the live HEAD tree
  before it pushes. Keep the public repo fresh — cut whenever a batch of lane
  work lands, not once.
  **Still NOT authorized**: mainnet anything, tags, releases, force-pushes,
  pushing any branch other than the cut to `main`, and reading wallet or
  private-key material outside the devnet keypair this work requires.
- Local commits are ordinary work. Add named files explicitly while parallel
  work is live.
- **Commit through `tools/lane.sh commit <msg> -- <paths>`, nothing else.** It has
  enforced `git commit --only` and read back the commit's actually-changed paths
  since 2026-08-30; on 2026-09-01 four lanes committed another lane's hunks by
  using `git add`/`git commit`/`-o` by hand instead. **Its known limit, learned
  the hard way:** `--only` commits the working-tree state of a path, so when two
  lanes edit *one file* it cannot separate their hunks — for that case use
  `tools/lane.sh commit-patch <msg> <patch-file>`, which applies your patch to an
  index it first proves empty, refuses if the staged path set differs from the
  patch's, commits HEAD's blob plus your hunk without `--only`, and names any path
  whose working tree still carries someone else's. Run `cargo check` on the touched workspace before any SBF build. If
  `.git/index.lock` is held, check `ps` for a live `git`/`op-ssh-sign` and wait.
- **The live tree is `/Users/ember/dev/dclutch`. There is a STALE NESTED COPY
  of it at `/Users/ember/dev/dragons-clutch/dclutch`** — the publication subtree
  host, frozen at whatever the last cut was. The shell's working directory
  **resets to `/Users/ember/dev/dragons-clutch` between commands**, so every
  relative path is one step from measuring the compost host instead of the tree.
  A lane lost a finding to this on 2026-09-01: it swept the nested copy at
  `3d7ac6fa6`, produced a confident verdict, and had to retract it.
  **Use absolute paths, and print `git rev-parse --show-toplevel` and
  `git rev-parse HEAD` in the same command as any measurement you intend to
  report.** A measurement whose tree root is not stated is not a measurement.
- Public RPC reads require an explicit task and must remain bounded.
- Never describe a deployment or frontend as official without a checked release
  manifest.

## Provenance

- Do not import, copy, or depend on code from JOSHI, joshibot, leanuweave,
  minidregg, breadstuffs, Oracle Pit, or historical DREGG prototypes.
- Dragon's Clutch code may be transplanted only through the process in
  `COMPOST.md`: name the invariant, source commit and path, license/provenance,
  new semantic owner, and adversarial tests. Prefer a fresh implementation from
  the written invariant.
- Never merge an old implementation merely to preserve sunk work. Git history
  grafting happens only after dClutch has a coherent independent architecture.

## Architecture

- Keep the universal Market Core small. Optional venues, liquidity, wrappers,
  bearer mints, and recovery depth are capability children, not universal
  ontology.
- Persist economic facts and replay authority, not an offchain workflow graph.
- One semantic owner per persisted fact. A separate concept does not
  automatically deserve a separate account.
- Market capabilities are immutable and canonically identified. Deferred
  physical creation must be precommitted and prepaid.
- Hoard principal is never fees, rent, bounty, insurance, work funding, reserve,
  or treasury capital. Future revenue is never present capitalization.
- Static clients, routers, matchers, and indexes are untrusted projections.

## Correctness vocabulary

- A state partition must be exhaustive, disjoint, ordered, and canonical before
  it can mint liabilities.
- Portfolio payoffs and simplex prices use exact scaled integers with one named
  rounding boundary.
- Say "best valid submitted candidate," never "optimal clearing," without a
  checked optimality certificate.
- Fixtures, simulation, local-validator execution, and devnet execution are
  distinct evidence levels and are not mainnet evidence.
- Do not call the protocol formally verified without naming the theorem, source
  digest, toolchain, assumptions, and unverified runtime boundary.

## Refusal codes

Every custom program error code is namespaced by program (decision 0007;
`crates/dclutch-refusal-registry` is the authority). `band = code >> 12`, and
band 0 is never allocated, so a code below `0x1000` is not ours.

- A refusal enum that can reach the chain carries `#[repr(u32)]` and explicit
  discriminants, written as hexadecimal literals inside its band, with a
  `const _: () = assert!(...)` pinning it to the registered base. `#[repr]` on
  an `*Error` enum is the declaration "these codes are protocol-visible"; the
  census enumerates nothing else.
- Never write a refusal code as a bare number anywhere else -- not in a test,
  not in a binding, not in a doc comment. Derive it from the enum, or from the
  registry base where taking a dependency on the program would be wrong.
  `assert!(text.contains("Custom(3)"))` is not a refusal assertion: it also
  accepts `Custom(30)`.
- **A bare `is_err()` is not a refusal assertion either, and a hostile test
  that asserts only `is_err()` is a test of nothing.** It passes on whatever
  the transaction refuses first, which during any wall era is a universal
  donor (ledger `M-38`). Measured twice on 2026-08-30: `67e96e5b` found four
  hostile `is_err()` assertions that had been "passing" for four days on a
  length refusal reached before any state was read, and `d1d1ff3f` found
  fifteen more hostile assertions naming no code at all — three of which
  refused somewhere other than where their author believed. Name the exact
  discriminant, derived from the enum, and prove the test red before trusting
  it green.
- **A bare `is_err()` has two causes, and only one of them is the test's
  fault.** The first is the hostile that never reaches its subject, above. The
  second is a hostile that reaches its subject and has no word for what it
  found, because the refusal it wants to name is one undifferentiated code over
  many conjuncts — and that one is fixed by SPLITTING THE DISCRIMINANT, not by
  rewriting the test. Measured on 2026-09-01: `4c90cdf5` split
  `InvalidScratchBank`'s six causes, and
  `corrupted_scratch_page_refuses_without_mutating_selection` — a bare
  `is_err()` that had survived every audit — became an exact
  `ScratchBankDigest` assertion in the same commit, predicted from the
  fixture's behaviour before it was run and confirmed by the run. Before
  reaching for the test, ask whether the code it needs exists.
- Bands are append-only. A new program takes the next free base; a deleted
  program's band is withdrawn, never reused.
- `dclutch-route-census inventory --check-unique` is the gate, and it runs in
  `tools/gauntlet/run.sh`.
- **`tools/genref/generate.sh` refuses a dirty tree (exit 3).** It reads the working
  tree, and in a shared checkout that emits reference docs for code not in HEAD —
  measured 2026-09-01, it silently deleted another lane's landed refusal rows.
  Regenerate from a detached worktree at HEAD; `--allow-dirty` only when you have
  read the diff and mean it. The convergence owner runs it; lanes do not.

## A `map_err` that discards its cause converts a located defect into a search

`map_err(|_| Coarse)` is the single most expensive idiom in this tree. The
callee already computed *which* conjunct failed; the wrapper throws that away
and publishes one code covering everything the call can refuse. Nothing goes
red, no gate notices, and the cost is paid later by whoever has to rediscover a
fact the program already knew — by bisecting a route that has no diagnostic.

Measured three times on 2026-09-01, in three subsystems, by three different
lanes, each costing hours:

- `programs/dclutch-trading-sbf/program-test/bundle-builder/src/registers.rs`
  discarded `execute_fold_atomic`'s error, so the transition phase published
  only `Projection("transition")`. Surfacing it gave `CheckFailed` with every
  register width equal — which ruled out the whole geometry family in one run.
- `programs/dclutch-trading-sbf/src/hot_v3.rs` discarded
  `ClaimsCompositionErrorV3` behind `TradingSbfError::Content`, so a route
  geometry mismatch surfaced as one of 2,124 possible `Content` sites.
  Surfacing it gave `Route` — "unsupported geometry or packet bytes" — and the
  conjunct was then two files apart and readable with no instrument at all.
- `programs/dclutch-trading-sbf/src/entrypoint_adapter.rs` did not discard an
  error but discarded a distinction, which is the same defect one level up:
  `require_extended_heap_admitted_v1` reads the ComputeBudget REQUEST and
  reports the GRANT. A request the runtime accepts and does not apply then
  faults instead of refusing.

**The recovery was identical every time: surface the inner error, then compare
declared against observed.** So:

- Prefer `map_err(|error| Specific::from(error))` over `map_err(|_| Coarse)`
  whenever the callee's error already distinguishes causes. If the wire cannot
  carry the distinction, keep the cause in a `msg!` on the refusing path — a
  validator log is where a reader looks first.
- A coarse code is legitimate when the causes are genuinely one accusation. It
  is not legitimate as a default, and "the caller only needs to know it failed"
  is how a route acquires 2,124 indistinguishable refusal sites.
- When you must localize one anyway, the cheap order is: use the tree's own
  instrument if it has one (`hot-cu-profile`'s 33 checkpoints), then markers
  between candidate callees, then re-run the same public entry point over
  successively longer prefixes so the answer comes from the authority rather
  than a second interpreter written in the harness. Degrade honestly: "could
  not be localized" is an output.
- **A probe measures what it touches, not what you meant.** A heap probe that
  ALLOCATES measures `entrypoint!`'s hardcoded `HEAP_LENGTH`, not the granted
  frame; only a raw write into the heap region measures the runtime. Verify the
  instrument before believing the reading.

## Kernel policy

The first-party kernel is `no_std`, `no_alloc`, safe Rust, fixed-layout, and
total. It forbids `unsafe`, FFI, floating point, dynamic allocation, unchecked
casts, proof-only preconditions, and hidden executable alternatives. Public
functions validate untrusted input and return explicit errors.

Solana SDK, Token-2022, oracle SDKs, CPI, account memory, RPC, and transaction
construction belong outside the kernel in explicitly named adapters.

## Project conduct

- Work from one canonical integration branch. Delegated lanes are short-lived,
  bounded, and either touch disjoint files or coordinate before editing.
- Never redirect a generator directly into a canonical tracked output. Emit to
  a temporary file on the same filesystem, require the producer to exit zero,
  validate the expected header/width (and formatting where applicable), then
  replace the canonical file atomically. A failed generator must leave the last
  accepted output byte-for-byte intact.
- Before every commit, inspect the complete staged path list. If the shared
  index contains another lane's files, stop and coordinate; a named-file `add`
  does not make a subsequent whole-index commit safe.
- **Never delete `.git/index.lock`.** In a shared checkout a lock that looks
  stale usually is not: 1Password's `op-ssh-sign` holds it for as long as
  another lane's commit is waiting on a signing prompt, which can be minutes and
  looks identical to a crash. Removing it corrupts that lane's in-flight commit.
  Check `ps` for a live `git`/`op-ssh-sign` first, then wait — measured
  2026-09-01, a lock that looked abandoned cleared on its own in nine seconds.
  If signing is what is stuck, an unsigned commit of your own paths is allowed
  and is the honest signal that a lane worked while Ember was away.
- Use `tools/lane.sh` for commits, pinned `rustfmt`, wave-board entries, and
  running a script another lane might edit concurrently. The raw commands
  (`git commit --only`, `rustup run 1.97.1 rustfmt --edition 2024`, appending
  to the board by hand) remain valid — the wrapper just carries the
  discipline (refusing empty/wildcard commit path lists, a post-commit
  readback, crate-root and mid-run-edit guards) so it isn't re-learned by
  hand each time. See `tools/lane/README.md`.
- **A backticked span in a shell-quoted message is command-substituted, and
  that includes BOARD POSTS.** zsh runs every `` `word` `` inside a double-quoted
  `git commit -m` *or* `lane.sh board` argument and silently substitutes the
  result, so a message loses exactly the code spans that made it precise. The
  commit hazard was known; that it applies to the board was not, and on
  2026-09-01 a board entry lost two passages this way and needed a correction.
  Write the message to a file and use `git commit -F`, or single-quote it and
  use no backticks — then READ BACK what actually landed. Which is the general
  rule: verify the instrument reported what you think it reported, not merely
  that it reported something.
- **Distrust silent success.** The worst failure mode in this tree is not a red
  gate; it is a command that reports success having done nothing, because every
  check downstream then measures an absence. Three instances, all measured on
  2026-09-01: `swarm-build` on hbox can fail with `Unit run-u<N>.scope was
  already loaded` and **still exit 0 having executed nothing** (hit seven times
  in one day; wrap it in a retry that greps for that string, and never read an
  empty exit-0 as a clean run). A long `cp -a` interrupted by a timeout leaves
  a partial directory, and a retry guarded by `test -d` then skips the recopy —
  use `rsync -a --delete`, which is idempotent. And a suite runner of bare
  invocations under `set -e` stops at its first failure, so the rows after it
  never run while the summary reports one number: `run-postjoin-hostiles.sh`
  reported one failing case when the true figure was ten. Run every row, report
  every row, and keep "failed" distinct from "never ran".
- **Program logs from one test binary interleave; isolate before attributing a
  refusal.** `cargo test` runs a binary's tests concurrently, and every one of
  them prints `Program log:` lines into the same stream. A refusal read out of
  that stream belongs to whichever test emitted it, which is not necessarily
  the one whose panic sits next to it. Measured 2026-09-01, by me, twice into
  commit messages: `freeze.rs` was reported as "width 1 commits, width 258
  refuses `0xC011 ScratchBankDigest`" when the `0xC011` came from
  `corrupted_scratch_page_refuses_without_mutating_selection` running
  alongside — a test that corrupts a page ON PURPOSE. Re-run with a name filter
  AND `--test-threads=1` before believing which test refused, and before
  believing a width, an ordering, or a count read from interleaved output.
- **Never point a nested program-test workspace at the root `CARGO_TARGET_DIR`.**
  Each `program-test` is its own workspace with its own `target/`. Overriding
  `CARGO_TARGET_DIR` to the root's mixes rustc invocations made from two
  workspace roots, so one path-dependency crate is compiled twice -- once with
  a relative source path and once absolute -- and the link fails with *"multiple
  different versions of crate `X`"* naming crates nobody touched. Measured
  2026-09-01: it blamed `dclutch-operator` and `dclutch-direct-codec` for a
  duplicate `dclutch-core-contract` while `cargo metadata` showed exactly ONE
  of it in the graph and uniform `../../../crates/...` spellings. Dropping the
  override built the same workspace in nineteen seconds. `cargo metadata`
  is the discriminator: if the graph has one copy, the manifest is innocent and
  the target directory is the culprit. Note that the general-hot command
  recorded in `docs/LETTER_TO_CLAUDE_2026_09_01.md` carries this override.
- **A commit carries the manifest and lock its code needs.** Named-path commits
  keep a shared checkout safe, and their exact cost is that the `Cargo.toml`
  and `Cargo.lock` a change depends on are easy to leave behind — three
  measured instances on 2026-09-01 alone: a `#[path]` relink committed with its
  dependency but not its lock; four workspace lockfiles that could not resolve
  under `--locked` at committed HEAD; and a program-test committed while the
  eight dev-dependencies it needs stayed uncommitted, which compiles for
  whoever holds the dirty file and for nobody else. Before committing, ask what
  the change now depends on that HEAD does not have. `cargo metadata --locked`
  answers it for a workspace in seconds, and nothing else in day-to-day work
  runs `--locked` at all.
- Build vertical executable slices. A slice includes kernel semantics, adapter,
  operator construction, and an honest user-visible status; no layer may claim
  completion alone.
- Do not substitute repeated audits for implementation. Reviews must end in a
  concrete decision, deletion, test, or queued implementation owner.
- Do not preserve parallel legacy/current authority paths. When a successor is
  accepted, delete the superseded path in the same convergence cycle.
- Banishing a program, crate, or route is not finished at the Rust boundary.
  In the SAME commit, sweep every non-Rust consumer for the deleted thing's
  vocabulary -- `apps/dclutch-web` above all, because a browser that mirrors a
  wire by hand becomes its LAST AUTHORITY the moment its owner is deleted, and
  nothing goes red. Grep the magics, the seed domains, the widths, the account
  counts, and the routes; delete or re-source each hit, and say in the message
  which cuts the banishment unblocked for other lanes. `npm run abi:coverage`
  lists what the browser still states in its own words; every generated module
  carries an `abi:*:verify` that `npm test` runs, so a surface with neither is
  a surface with no authority behind it.
- **A commit changing any crate compiled into an SBF link carries its
  frameguard baseline rows or says in its message that it leaves the ratchet
  red** -- the link is its whole path-dependency closure, not its program
  crate, so a codec change three edges down owes rows too; the exact
  per-function frame manifest cannot be recaptured after the fact by a
  bystander, because the double build is longer than this tree's interval
  between program commits (three correct recaptures were each invalidated
  within minutes on 2026-09-02), so capture with `tools/frameguard/run.sh --at
  <commit>` in the same commit, and `frameguard.py owed --repo . --baseline
  tools/frameguard/baseline.json` names who has not.
- Every fixed bound is labeled as mathematical, chain-derived, measured-profile,
  or provisional. Provisional bounds require a lifting plan.
- Add adversarial tests with each invariant or parser. Do not weaken a refusal
  to make an integration test pass.
