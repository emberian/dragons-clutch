# Operator Bench — the first frontend

Status: **DESIGN / APPROVED FOR BUILD** (2026-08-21, the frontend-design
lane, restated here by the implementing lane from the brief plus its own
read of the ground truth: `programs/clutch-sbf/harness/src/main.rs`,
`programs/clutch-sbf/committed-harness/src/main.rs`,
`programs/clutch-sbf/scripts/run_general_committed.sh`,
`apps/static-client/app.js`, `site/style.css`).

## The thesis

Everything this repository can honestly show, it can already show *only*
as a scrolling terminal log. `run_general_committed.sh` boots a local
validator, drives forty-four signed transactions through the sealed
non-production mock-source ELF, reloads exact account bytes at every
step, waits on the validator's real clock twice, and re-derives the
terminal conservation identities from the bytes it observed. That is the
strongest evidence the project has at the runtime plane, and it is
invisible unless you read 11.8k lines of Rust and a 12k-line log.

The Operator Bench is a *window onto that exact walk* — not a
reimplementation of it. The rule that makes it honest is structural
rather than editorial:

> **The browser never builds a transaction.** It has no RPC client, no
> wallet, no serializer, and no key material. It receives decoded state
> and a step log over one server-sent event stream, and it POSTs
> intents. Every byte that reaches the bank is produced by the same
> `clutch_sbf_harness` builders the sealed lane uses.

If the page could construct a transaction, the page would become a second
implementation of the wire format, and the first divergence between it
and the harness would be invisible. Instead the divergence is
*impossible*: there is one builder, and the replay falsifier (Step 3)
proves it by byte-diffing the daemon's rebuilt transactions against the
harness's own emitted files and requiring a corrupted byte to go red.

## What this is evidence for, and what it is not

The Bench inherits the general-clearing lane's scope exactly and widens
it in no direction:

- **SBF-EXECUTED, unpromoted.** Signed, confirmed, committed sequential
  execution on a local `solana-test-validator` from a genesis-assisted
  prestate, against an ELF built with `--features
  non-production-mock-source`.
- **NOT** a deployment, devnet evidence, mainnet evidence, a wallet, an
  operatorless venue, or a blank-bank lifecycle. The plan preloads six
  program-owned prerequisites and the runner prints `NOT END TO END`
  before it submits anything.
- **NOT** verified. The UI's own prose never uses the word. Claim
  vocabulary is reused verbatim from the frozen `EVIDENCE` map in
  `apps/static-client/app.js`; the Bench introduces no new claim kind.

The honesty surface is therefore *structural*, not a footnote: a
permanent, non-dismissible header strip carrying `NON-PRODUCTION
mock-source ELF`, the live-hashed ELF sha256, `LOCAL 127.0.0.1 ONLY`, `no
value`, and the evidence scope `SBF-EXECUTED (unpromoted)`. It cannot be
closed, it is rendered before any data arrives, and it is re-rendered
from the daemon's own hash of the `.so` it actually loaded — not from a
constant in the page.

## Step 1 — the harness library split

`programs/clutch-sbf/harness/src/main.rs` (11 836 lines) becomes
`src/lib.rs`; `src/main.rs` becomes a nine-line argument forwarder to
`clutch_sbf_harness::run_cli`. The library exposes, as its published API:

| surface | items |
| --- | --- |
| PDA derivation and encodings | `Pda`, `derive`, `fixed_address`, `fixture_identity`, `base58_of`, `b58_decode32`, `b64_encode`, `hex_encode` |
| the fixture / plane genesis model | `Fixture`, `build_fixture`, `Shared`, `build_shared`, `Plane`, `Walk`, `Batch`, `GeneralActors`, `build_general_actors`, `GeneralOwner`, `owner_plane`, `owner_view_plane`, `wallet_pda`, `founding_plane` |
| request bodies | `layout_request`, `resolve_request`, `redeem_request`, `redeem_external_request`, `endow_request`, `endow_request_for`, `withdraw_cash_request`, `create_intent_bytes` |
| message plumbing | `Message`, `Instruction`, `transaction`, `compact_u16` |
| budget and heap riders | `budget_instruction`, `heap_frame_instruction`, `COMPUTE_UNIT_CEILING`, `REQUEST_HEAP_FRAME`, `HEAP_FRAME_BYTES` |
| per-family transaction and account-meta builders | `seam_leg_accounts`, `seam_instruction`, `seam_message`, `seam_transaction`, `gate_transaction`, `paired_resolve_transaction`, `endow_transaction`, `endow_transaction_at`, `withdraw_cash_transaction`, `advance_transaction`, `create_transaction`, `general_transaction` + `GeneralTx`, the token-leg transactions |
| the conservation-offset table | `GeneralConservation`, `position_field_offsets`, `hoard_collateral_offset`, `sole_u64_offset` |
| plan emitters | `emit_cases`, `emit_committed_plan`, `emit_general_committed_plan`, `build_general_committed_cases` |

**The split gate.** Emit `--general-clearing` into a fresh directory
before the split and again after it, and require the two trees to be
byte-identical — every `tx/*.b64`, every `expected/*.hex`, every
`accounts/*.json`, `genesis.txt`, `committed.json`, and the emitter's own
stdout. The fixture is key-free by construction when the
`CLUTCH_COMMITTED_*` variables are absent (`fixture_identity` falls back
to a System-program PDA of a literal seed), so the comparison is exact
and repeatable rather than approximate.

## Step 2 — `operatord`, M0 "watch mode"

`programs/clutch-sbf/operatord/`, a Rust loopback daemon.

**Workspace placement.** `operatord` is its own workspace, exactly as
`committed-harness` is, and for the reason `committed-harness/README.md`
already records: *the program ELF must not acquire host signing or JSON
dependencies merely because a local evidence runner needs them.* The SBF
workspace lock is 42 packages; joining it would drag `solana-keypair`'s
signing graph into the lock that the sealed baseline resolves the ELF
from. `operatord` path-depends on `../harness` and `../program` across
the workspace boundary, so it still calls the one true builder without
moving the ELF's dependency pins. **No member line is added to
`programs/clutch-sbf/Cargo.toml`.**

**Lifecycle.** `operatord serve` reproduces `run_general_committed.sh`
step for step, in process:

1. mint fresh test-only keys into a `mktemp` directory
   (`payer`, `actor`, `owner-b`, `owner-c`, `owner-d`, and the three
   ordinary collateral-token identities), unlinked on exit;
2. build the ELF with `--features non-production-mock-source`, hash it,
   and report bytes and sha256;
3. emit the general-clearing plan by calling
   `clutch_sbf_harness::emit_general_committed_plan` **in process** with
   the fresh public keys exported — the same code path the script shells
   out to;
4. start `solana-test-validator` with `--reset`, the program ELF, and one
   `--account` per genesis row, on an operator-chosen loopback RPC port;
5. probe readiness exactly as the script does (slot >= 1 **and** the
   program account present and `executable`);
6. drive the forty-four steps through the committed-harness code path —
   sign, submit with preflight disabled, await `confirmed`, reload every
   compared account, snapshot watched accounts around refusals, apply
   slot patches, honour `wait_slot` and `wait_after`;
7. re-derive the conservation identities from the observed bytes and
   publish the epilogue.

**HTTP surface** (loopback bind only; a non-loopback bind is refused at
startup, mirroring `require_loopback`):

- `GET /` and the static files of `apps/operator/`, served from an
  allowlisted extension set with no path traversal;
- `GET /api/events` — server-sent events. One event per lifecycle
  transition, in the committed-harness log grammar (`ordinal`, `name`,
  `kind`, `confirmationStatus`, `slot`, `cu`, `reloads`, `signature`),
  plus `state` events carrying decoded account bytes through the layout
  codecs, `clock` events carrying the bank slot during the two real-clock
  waits, and a terminal `conservation` event;
- `POST /api` — actions. M0 needs exactly two: `start` and `stop`.

**Screens (M0).**

*The Bench.* Validator health (pid, RPC port, current slot, ledger
path), the ELF identity block (path, bytes, live sha256, source profile,
`NON-PRODUCTION` banner), the actor roster (payer + four signing owners
with their public keys and roles), and the lifecycle rail — forty-four
steps as a vertical spine, each in one of five states: pending, waiting
on clock, in flight, accepted, refused-as-expected.

*The Walk.* The step log, one row per step, with a per-step compute-unit
bar measured against the 1 400 000-unit transaction ceiling
(`COMPUTE_UNIT_CEILING`), the confirmed slot, the reload count, and the
signature. Refusal steps are **first-class**, not errors: they render
with their expected `Custom(0x….)` code and the offline reference's own
refusal text, because "the program refused this, exactly here, with
exactly this code" is the evidence. The two real-clock waits render a
live slot countdown driven by `clock` events.

**Browser constraints.** Zero dependencies. Hand-authored ES modules, no
build step, no bundler, no transpile. `site/style.css`'s token block is
copied to `apps/operator/styles.css` so the Bench reads as the same
project as the microsite. No `http://`, `https://`, or CDN reference
appears anywhere under `apps/operator/` — grep is the gate.

## Step 3 — the replay falsifier

`operatord replay <plan-dir>` rebuilds every transaction of the plan
through the daemon's own builders — i.e. through `clutch_sbf_harness` —
and byte-diffs each against the `tx/*.b64` file the harness emitted. It
then corrupts one byte of one rebuilt transaction and requires the
comparison to go red. `programs/clutch-sbf/scripts/run_operator_replay.sh` runs both halves and
fails if either verdict is wrong — it lives beside `run_general_committed.sh`
rather than in the repository-root `scripts/`, which holds the Python
manifest tooling and no shell gates.

This is the gate that makes the "one builder" claim checkable rather than
asserted. It is *not* a proof about the wire format; it is a byte
comparison between two callers of one function, and it is described at
that resolution.

## Step 4 — M1

- The Friday-clutch eight-outcome degree-1 fixture, modelled on
  `svm-tests/tests/disagreement_exhibit.rs`'s T0 terms (basis_degree 1,
  knot_count 8, u128 cent knots, `STAT-TERMINAL-01`, `EDGE-CLAMP-01`).
- Interactive Endow / Split / PlaceOrder builders behind `POST /api`,
  each landing in the same `general_transaction` path.
- Funding, Ticket, and Book screens. The density painter is **M2**; a
  plain per-knot limit ticket is enough for M1.
- Auto-crank with the visible step log.
- The live conservation strip, re-derived from observed bytes on every
  SSE tick rather than read from the plan's `expected` block.

## Gates

1. Split gate — the emitted `--general-clearing` plan is byte-identical
   before and after the library split.
2. `programs/clutch-sbf/scripts/run_operator_replay.sh` green, and red on a
   corrupted byte.
3. M0 end to end under the suite lock, by
   `programs/clutch-sbf/scripts/run_operator_bench.sh`: the full
   forty-four-step walk watched through the API as a client, conservation
   epilogue green.
4. `cargo clippy --all-targets -- -D warnings` on `clutch-sbf-harness`
   and `clutch-sbf-operatord`.
5. Zero external dependencies in `apps/operator/`, by grep.

## Delivered

Built 2026-08-21 by the implementing lane, in the shared tree on `main`.

**Step 1, the library split** — `harness/src/main.rs` (11 836 lines) is now
`src/lib.rs` plus a nine-line `src/main.rs` forwarding to `run_cli`. The
split gate passed: the `--general-clearing` plan emitted before and after
was byte-identical across all 294 files (tree digest
`79452e9ef4215562fd040d38aba829aa51c49380de29b0d2cc69c09033e83d11`), with
the emitter's stdout matching line for line. The plan's bytes have since
moved with the partial-fill lane's `ReservationAccount` v2 (570 → 610); the
gate above was taken with both emissions at one tree state and is unaffected.

**Step 2, `operatord` M0** — `programs/clutch-sbf/operatord/`, its own
workspace, path-depending on `../harness` and `../../solana-layout`. No
member line was added to `programs/clutch-sbf/Cargo.toml`. `serve` mints the
eight ephemeral signers, builds and hashes the mock-source ELF, emits the
plan in process, starts a fresh ledger, probes readiness, and drives the
forty-four steps, publishing `boot` / `roster` / `identity` / `plan` /
`step` / `state` / `clock` / `crank` / `conservation` / `done` events.

**Step 3, the replay falsifier** — `operatord replay` plus
`run_operator_replay.sh`. Verdicts: the library plan and the CLI plan are
byte-identical (294 files, 44 transactions); every file rebuilds
byte-identically through the builders; a single corrupted transaction byte
turns the replay red at exactly the corrupted file.

**Step 4, M1** — the live conservation strip (re-derived from observed bytes
on every tick, with unobserved roles named rather than zeroed), the Funding,
Ticket and Book screens, and the crank. **Not built:** interactive Endow /
Split / PlaceOrder builders, the Friday-clutch eight-outcome degree-1
fixture, and the density painter. The crank is pacing only — pause between
steps, take exactly one, resume — and `POST /api` deliberately has no verb
that composes, reorders, or skips a transaction, so the reading surface
cannot become an authoring surface.

### One deviation from the brief, and why

The brief allotted one member line in `programs/clutch-sbf/Cargo.toml`. It
was not used. `committed-harness/README.md` already records the rule — *the
program ELF must not acquire host signing or JSON dependencies merely because
a local evidence runner needs them* — and that workspace's 42-package lock is
what the sealed baseline resolves the deployable artifact from.
`solana-keypair`'s signing graph has no business in it. `operatord` is
therefore a sibling workspace, exactly as `committed-harness` is, and the
path dependency across the boundary still gives it the one true builder.
This also removed the anticipated conflict with the keeper lane.

### Gate results

| gate | result |
| --- | --- |
| split byte-diff | **PASS** — 294 files identical, digest `79452e9e…83d11`, stdout identical |
| library plan == CLI plan (current tree) | **PASS** — 294 files, 44 transactions, byte identical |
| `run_operator_replay.sh` | **PASS** — 0 differing; a corrupted transaction byte goes red at exactly that file |
| clippy `-D warnings`, both crates | **PASS** |
| unit tests (`operatord`) | **PASS** — 20 |
| no external reference in `apps/operator` | **PASS** — grep finds nothing |
| manifest-license / dependency-license audits | **PASS** with the new crate in the tree |
| **M0 end to end** | **FAIL at step 40, on an inherited tree break** |

The M0 walk (run at `24428fc`, under the suite lock, RPC 9137 / faucet 9138 /
bench 9130) drove **39 of 44 steps**: 37 accepted, 2 refused with their exact
codes, 97 account reloads compared byte for byte, 77 of them decoded through
the layout codecs, both real-clock waits honoured (`plan deadline` to slot 800
and `general-28-freeze-epoch + 1000`), 3 587 clock ticks and 39 live
conservation strips published, peak 331 100 CU at
`general-34-advance-pass-one` against the 1 400 000 ceiling.

It then stopped, correctly, at:

```
general-40-entitle-single-slice / general.reservation-2: committed bytes differ
(1 of 610 bytes differ; first at offset 570: observed 0x08, expected 0x00)
```

Offset 570 is exactly the boundary between `ReservationAccount` v1 (570 bytes)
and the partial-fill lane's v2. The program stamps the appended field —
`direct_selection.rs:935-936`, `buy_reservation.entitled_units =
facts.quantity` — and the harness's host-side expectation does not:
`harness/src/lib.rs` contains no occurrence of `entitled_units`, and lines
10164-10165 / 10257-10258 set `state = RESERVATION_STATE_ENTITLED` without
stamping it.

This is **not** an Operator Bench defect and it is not caused by the library
split. `run_general_committed.sh` drives the same plan against the same ELF
through the same byte comparison, so the sealed lane is red at the same step
for the same reason. The daemon detected it and refused, which is the
machinery working. Fixing it means deciding what `entitled_units` and
`consumed_units` are at each entitlement and consumption step — the
partial-fill lane's semantics, deliberately not patched from here.
