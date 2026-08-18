# Handwritten Rocq kernel shadow

Status: **SPECIFICATION ADDED; Rocq/Coq CHECK UNAVAILABLE** (2026-08-18)

[`rocq/ClutchKernel.v`](../../rocq/ClutchKernel.v) is a Rust-independent pure
state machine for the landed `clutch-kernel` surface.  It models:

- `MAX_OUTCOMES = 16`, `MIN_OUTCOMES = 2`, and `MAX_PAYOUTS = 8`;
- payout vectors with a positive common denominator, active-length weights,
  nonnegative bounded weights, and an exact weight sum;
- active maximum liability using ceiling division and resolved liability using
  the selected finite payout vector;
- Hoard collateral and conservative per-outcome total supply;
- complete-set `split` and `merge`;
- supply-neutral `materialize` and `dematerialize`;
- finite indexed `resolve`;
- internal and external `redeem`, including refusal when
  `quantity * weight` has a nonzero denominator remainder.

Lists are the mathematical representation of active prefixes of the Rust
kernel's fixed arrays.  `state_validb` checks their lengths and all bounded
amounts.  Every operation is total and returns `None` on invalid shape,
phase, quantity, balance, arithmetic bound, payout index, or post-state
invariant failure.  The model uses unbounded Rocq `nat` for intermediate
arithmetic and explicitly bounds stored amounts to `U64_MAX`; the exact Rust
`u128` intermediate-width proof and the correspondence to fixed-width Rust
are named follow-up obligations, not silently assumed as proved.

The file contains no `Admitted`, `Axiom`, extraction, Rust translation, or
network/tool installation.  The named properties at its end are `Prop`
definitions only: no theorem is claimed checked or proved.  In particular,
the following remain open obligations:

1. successful transitions preserve `state_validb` from a valid pre-state;
2. split/merge preserve the maximum-liability solvency inequality;
3. materialize/dematerialize preserve total supply and collateral exactly;
4. exact redemption pays the represented quotient and refuses every remainder;
5. the finite-prefix model refines the Rust fixed-array/adapter boundary;
6. all Rust `u128` checked-product and checked-sum bounds are represented;
7. reachable-state initialization and multi-position aggregate accounting are
   equivalent to the kernel's authenticated adapter contract.

Local probe result:

```text
$ rocq/check.sh
status=UNAVAILABLE
reason=no rocq or coqc executable found on PATH
```

The script is confined to `rocq/`, uses no network or package manager, copies
the source to a temporary directory when a compiler is present, and exits 2
for the explicit unavailable state.  No release, refinement, deployment,
RPC, key, financial, or formal-verification claim follows from this artifact.
