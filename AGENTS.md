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
- Local commits are ordinary work. Add named files explicitly while parallel
  work is live.
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
- Bands are append-only. A new program takes the next free base; a deleted
  program's band is withdrawn, never reused.
- `dclutch-route-census inventory --check-unique` is the gate, and it runs in
  `tools/gauntlet/run.sh`.

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
- Use `tools/lane.sh` for commits, pinned `rustfmt`, wave-board entries, and
  running a script another lane might edit concurrently. The raw commands
  (`git commit --only`, `rustup run 1.97.1 rustfmt --edition 2024`, appending
  to the board by hand) remain valid — the wrapper just carries the
  discipline (refusing empty/wildcard commit path lists, a post-commit
  readback, crate-root and mid-run-edit guards) so it isn't re-learned by
  hand each time. See `tools/lane/README.md`.
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
- Every fixed bound is labeled as mathematical, chain-derived, measured-profile,
  or provisional. Provisional bounds require a lifting plan.
- Add adversarial tests with each invariant or parser. Do not weaken a refusal
  to make an integration test pass.
