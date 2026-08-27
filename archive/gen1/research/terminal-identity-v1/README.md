# TerminalIdentityV1 header

Status: **PROPOSED / MODEL-ONLY**, pending ember's ratification of the R4
terminal lifecycle design. This crate is interim step 3 of
[`docs/design/TERMINAL_LIFECYCLE_RUNTIME_V1.md`](../../docs/design/TERMINAL_LIFECYCLE_RUNTIME_V1.md)
§9; §1 of that design is controlling for everything here.

**No account family uses this header yet.** The crate changes no SBF
program, account layout, mint, authority, or classification row, and it is
wired into no existing crate or gate. As a brand-new crate it has **no
declared manifest gate** — that is deliberate: folding it into the declared
gate inventory happens at the next baseline manifest emission cycle, not by
hand-editing the current baseline.

## What it is

The uniform persisted rent/donation header every *new account version* would
carry under the R4 design, plus a lifecycle value model whose economics are
delegated entirely to the `clutch-liveness` `DonationLedger` kernel
(`admit_prefunded` / `observe` / `terminal_split`). Nothing here invents an
economic judgment the kernel does not already make.

### Header layout (56 bytes, fixed, no magic, no padding)

| offset | size | field             | encoding                                     |
|-------:|-----:|-------------------|----------------------------------------------|
|      0 |   32 | `payer`           | `[u8; 32]` exact funding wallet               |
|     32 |    8 | `payer_principal` | `u64` LE, exact lamports debited              |
|     40 |    8 | `donation_floor`  | `u64` LE, monotone DonationLedger lower bound |
|     48 |    8 | `generation`      | `u64` LE, close/reopen and replay era from 1  |

Decode refuses truncation and trailing bytes (exact length only), a zero
payer key, a zero principal, a zero generation, and a payer equal to the
neutral sink.

### The neutral sink is a parameter

Every codec and lifecycle entry point takes the frozen neutral sink as an
argument. The design's decision 10 pins the runtime value to one frozen
program-wide sink: the **incinerator**
(`RESOLUTION_WORK_NEUTRAL_SINK_V1 = incinerator::ID` generalizes). This
model crate does not restate that constant; it enforces only the relations
the header can check.

### Lifecycle semantics (design §1)

- `create`: artifact-stage transfer/allocate/assign —
  `payer_principal = rent_shortfall`; a pre-existing balance seeds
  `donation_floor` via `admit_prefunded` and is never credited to the payer.
- `observe_transition`: re-runs the kernel `observe`; surplus accretes
  monotonically into the donation compartment and is never reclassified. The
  accounted compartments must cover the principal.
- `close`: once, ever, per generation — pays exactly `payer_principal` to
  the **stored** payer and routes the entire remaining surplus through
  `terminal_split` to the neutral sink; a second close refuses. At close the
  accounted amount must equal the principal exactly (economic close strictly
  precedes rent close), so `payer_principal + neutral_surplus` equals the
  closing balance with no remainder.
- `reopen`: only after close; advances `generation` by exactly one and
  admits a fresh ledger, possibly under a different payer, with the sink
  carried over unchanged.

## Falsifiers

`tests/falsifiers.rs` states the design claims adversarially:

- a prefund can never reach the payer;
- donations accrete monotonically and never reduce `payer_principal`;
- close conservation is exact, no remainder, over an exhaustive small grid;
- a deficit (`actual < accounted + donation_floor`) refuses rather than
  clamps.

## Run

```sh
cargo test --manifest-path research/terminal-identity-v1/Cargo.toml
cargo clippy --manifest-path research/terminal-identity-v1/Cargo.toml \
  --all-targets -- -D warnings
```
