# Succinct clearing verification — feasibility finding

Status: **FEASIBILITY FINDING / PROPOSED DIRECTION.** Read-only scouting of
the adjacent `breadstuffs` certificate stack against Dragon's Clutch's
measured compute wall (Direct V2 selection consumes exactly 1,400,000 CU and
rolls back). This document promotes nothing and wires nothing. It records
what was verified, what is measured, what is not, and the two conditions
that gate the direction.

**PREMISE CORRECTED 2026-08-19 (same day):** the compute wall named above
dissolved. It was a software SHA-256 hasher, not the verification
architecture
([COMPUTE_CEILING_REATTRIBUTION_2026-08-19.md](../reviews/COMPUTE_CEILING_REATTRIBUTION_2026-08-19.md));
on the resealed syscall-hashed artifact, V2 selection completes and commits
at 226,071 CU. Succinct verification is therefore no longer motivated by
this measurement. It remains motivated for book widths where even staged
re-execution outgrows the budget, and every finding below about the
breadstuffs stack stands unchanged on its own evidence.

**Substrate, stated up front:** AIR / constraint systems are authored in
Lean, where refinement to spec is a machine-checked theorem over the
actually-emitted object; Rust only calls into the emitted artifact. Any
direction below inherits that rule without exception.

Prior art re-verified rather than rediscovered:
`docs/implementation/CERTIFICATE_STACK_INVENTORY.md` (2026-08-18),
`docs/implementation/OPTIMALITY_CERTIFICATE_MAPPING.md`, and
`docs/research/DUAL_IS_THE_MEASURE.md`. The findings there hold.

## 1. Verdict

Succinct on-chain verification of a Dragon's Clutch clearing is **plausible
with the stack that already exists**, subject to two named conditions in §4.
It is not blocked by cryptographic architecture. The construction that works
is the one the adjacent tree already built: a STARK over a Lean-emitted AIR,
recursively shrunk, Groth16-wrapped onto `alt_bn128` syscalls.

## 2. Why the semantics already match

The prior-art concept audited here is integer LP primal-dual certification: a
certificate establishes that every feasible alternative flow has objective at
most the certified flow's objective plus a named epsilon. This is a
source-independent mathematical statement, not an imported theorem
declaration. An exact Lean declaration that a historical draft copied from the
adjacent repository was removed on 2026-08-22 to restore the greenfield
boundary.

That is verify-not-find as a theorem: the untrusted solver's search is never
re-examined, only its output certificate is checked, at `O(m + nnz A)`
instead of `O(T·m)`. Dragon's Clutch independently derived the same object
from the other direction — `DUAL_IS_THE_MEASURE.md` records that every
accepted candidate already carries a zero-duality-gap dual certificate with
`π = p`. Two trees converged on one object.

The refinement theorem is over the emitted descriptor, in the security
direction (`CertFDescriptor.lean:1491`): any assignment whose trace
satisfies the deployed IR-v2 denotation of the Lean-emitted descriptor
carries a genuine `Market.Certified` certificate. Zero `sorry` on that path,
axiom-audited over 37 named theorems, with refusal teeth exhibited (a
mutated dual slack producing gap 250,000 against ε = 2,000 is proved to
admit no satisfying trace under any hash). The modular-to-integer boundary
is carried as explicit hypotheses with wrap counterexamples, not laundered.

## 3. The measured numbers

| Quantity | Value | Source |
| --- | --- | --- |
| On-chain Groth16 verify | **~255,000 CU** (syscall floor 230,283 + 15–25k) | pinned against `solana-program-runtime` execution budget |
| Transaction size | **795 bytes** of 1,232 | `solana-settlement/tests/settle_flow.rs:270` |
| Marginal cost per public input | ~4,174 CU | one `alt_bn128` mul + add |
| Raw STARK proof | ~120 KiB | ~100x over the packet limit — the wrap is mandatory |
| Prover, per batch, off-chain | 264 s apex + 105 s shrink + 16.7 s Groth16 | recorded cutover measurements |

Against the 1,400,000-CU wall that killed V2, ~255k CU is a **5.5x margin**,
in a transaction with 437 bytes to spare. This refutes "not possible on
Solana" outright: the verifier exists, is native SBF, and passes tests.

## 4. The two gating conditions

1. **The trusted setup is not discharged.** The current verifying key comes
   from a **dev single-party ceremony with known toxic waste**, stated as
   such in `solana-settlement/src/lib.rs`. Anyone holding it can forge any
   statement. No venue may ship on it. This is a ceremony problem, not a
   code problem, and it is the gating item.
2. **The Cert-F to Groth16 link does not exist.** The existing wrap consumes
   a 25-lane whole-history turn statement; the apex/shrink path contains
   zero references to Cert-F. Wiring it requires per-program Lean emission,
   a byte pin, a registry entry, and a discharged integer-admission
   (no-wrap) proof for Dragon's Clutch's actual clearing program. The two
   registered programs today are a three-edge ring and a four-order market
   — toys. Nothing demonstrates Cert-F at this project's batch width.

## 5. Corrections to project folklore

Two long-repeated cautions were verified and are **overstated as usually
phrased**; the accurate versions:

- The forked FRI patch is a **completeness** defect, not a soundness hole:
  the upstream bug causes a valid proof to be *rejected*, never a forged one
  to be accepted. The equivalent upstream fix is open with changes
  requested. The real cost is owning a divergence upstream declined, where a
  revision bump silently reintroduces the bug with its guard already
  deleted. Do not say "unmerged soundness fix."
- The restricted-license vendor concern is real but **off this path**: the
  patent-encumbered Zama crates reach only the FHE crate under an
  off-by-default feature. The vendored FRI is dual Apache-2.0/MIT. Scoping
  to Cert-F makes the question moot. Two vendored directories do ship
  without their declared license files; that must be fixed before any code
  crosses trees.

## 6. Debt surfaced

`fhegg-solver/src/air.rs` in the adjacent tree is a **hand-written Rust AIR**
authoring the same Cert-F rows the Lean descriptor authors — a vestigial
twin, off the STARK path. It is debt to be surfaced, never a foundation.
Dragon's Clutch itself contains no hand-written AIR; that ledger is clean
and must stay clean.

## 7. What is unmeasured (and must not be estimated)

- Cert-F proof size and prove time at Dragon's Clutch batch width. The
  existing 120 KiB figure is for the descriptor interpreter, not Cert-F, and
  the Cert-F tests are functional only — never instrumented.
- Whether the Cert-F statement survives recursion into the apex without
  exceeding the measured shrink budget.

Either could change the verdict. Neither is estimated here because no
measurement exists.

## 8. If this direction is taken

It needs one semantic owner, and its first unit is a measurement, not an
implementation: emit a Dragon's Clutch clearing program at realistic batch
width, and measure proof size, prove time, and shrink feasibility. The
ceremony question should be opened in parallel because it has the longest
lead time. Nothing about this displaces Direct V3, which remains the unlock
that makes the smooth claim family tradeable at all.
