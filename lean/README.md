# The Lean model of the kernel's semantic plane

Status: **MODEL.** Built, zero `sorry`, zero project axioms. This directory
holds a Rust-independent mathematical model of the semantic plane of
`crates/clutch-kernel`, and the theorems proved about it.

Architecture, the claim shape, the toolchain pin, the findings against
`docs/implementation/DISTRIBUTIONAL_CLAIMS_DESIGN.md` §3.2, and the ranked next
theorems are in
[`../docs/implementation/LEAN_MODEL_PLAN.md`](../docs/implementation/LEAN_MODEL_PLAN.md).
Read that first; this file is the map and the commands.

## The claim shape

> Lean 4.33.0 checked theorem `T` about the model `M` in `lean/` at source
> digest `d`, under hypotheses `H`. `M` is a hand-written mathematical model of
> the kernel's semantic plane. Its correspondence to `crates/clutch-kernel` is
> manual, unproved, and bounded only by the semantic vectors both evaluate. No
> theorem in `M` is a statement about the Rust program, the compiled SBF ELF, or
> any deployed program.

Nothing here is a verified implementation. Nothing here is release evidence.

## Map

```text
DragonsClutch/Basic.lean        amounts and the bound, ceiling division, dot, max, uniform shifts
DragonsClutch/Basis.lean        payout vectors, admissibility (H1)/(H2), basis families, preset sets
DragonsClutch/Solvency.lean     the liability functionals and the central theorems
DragonsClutch/Kernel.lean       market state, position, the ten transitions
DragonsClutch/Transitions.lean  the transition-level theorems
DragonsClutch/Vectors.lean      two canonical semantic vectors, evaluated and checked by `#guard`
```

The headline theorems, by property ID:

- `P_SOLV_01_resolution_bound` — resolution can never raise the collateral
  requirement, for every weight vector with nonnegative weights summing to `D`.
- `P_SOLV_01_required_active_is_exact_sup` — `max_i T_i` is the *exact* supremum
  over the frozen simplex lattice, attained, not a chosen over-reservation.
- `P_SOLV_01_resolve_with_vector_admits` — a solvent Active market always
  accepts an admissible vector; the prospective invariant check is defence in
  depth, never a live refusal.
- `P_PAY_02_complete_set_never_stranded` — a complete-set holder always exits,
  paid exactly `q`, at every resolved value, in either mode.
- `P_PAY_01_liability_fits_u128` — the partition of unity is what makes the
  kernel's `u128` liability accumulator unable to overflow.
- `R8_merge_collateral_refusal_is_ordering_artifact` — `merge`'s
  `insufficientCollateral` is reachable only where the balance test also fails.

## Build

```sh
cd lean && lake build
```

Expected: `Build completed successfully`, zero errors, zero warnings. No network
access, no dependencies — `lake-manifest.json` has `"packages": []`.

## Audits

Axioms (the only acceptable answer is Lean's own three, or fewer):

```sh
cd lean
printf 'import DragonsClutch\n#print axioms DragonsClutch.P_SOLV_01_resolution_bound\n' > /tmp/ax.lean
LEAN_PATH=.lake/build/lib/lean lean /tmp/ax.lean
```

Forbidden constructs (expected: no hits outside prose):

```sh
grep -rn "sorry\|axiom\|native_decide\|unsafe\|@\[implemented_by\]" DragonsClutch/
```

## The rule this directory lives under

A theorem is worth having only if its *statement* is worth reading. A vacuous
theorem, a theorem about the input state where the output was meant, or a
theorem weakened until it was provable, all build green and are all worse than
a named obstruction. See the plan's §8 for what failure looks like here.
