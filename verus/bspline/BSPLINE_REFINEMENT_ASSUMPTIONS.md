# B-spline finite refinement assumptions

The checked result depends on all of the following:

1. Lean 4.33.0 and its kernel correctly check the imported theorems and
   evaluate the fixture definitions; the model source contains no `sorry`,
   `admit`, project `axiom`, or `native_decide` in the checked closure.
2. The hand-written Lean definitions express the intended open-clamped
   `BasisFuns` and `WEIGHT-ROUND-01` semantics.  The generic uniform-knot
   linkage is proved in Lean, but associating each fixture's concrete `Split`
   literals with its CSV knot/value row is reviewed and finite, not a general
   parser/refinement theorem.
3. `emit_fixtures.lean` is only a faithful serializer of
   `bsplineRefinementFixtures`; its digest and exact output transcript are
   pinned and reproduced before Rust is run.
4. `examples/oracle_driver.rs` faithfully maps its CSV rows to `BasisSpec`
   without changing evaluator semantics.  The driver is digest-pinned but is
   not formally verified.
5. `BasisSpec::evaluate` in the pinned complete production `lib.rs` is the
   production semantic seam.  The campaign does not copy its recurrence or
   largest-remainder loop into an evidence implementation.
6. Rust 1.98.0-nightly correctly compiles and executes the crate on the host,
   and its integer and control-flow behavior agrees with the compiler used by
   any later consumer.  No SBF compiler or emitted ELF is covered.
7. Each named temporary mutation changes only the intended source occurrence;
   the runner requires the mutant to compile, execute all eight rows, and
   disagree with the Lean transcript.
8. SHA-256 collision resistance is adequate for the source, transport,
   emitter, and transcript drift pins.

The result assumes no Solana account, source/archive, statistic, signer,
Token-2022, SBF VM, runtime, deployment, or network fact.  It does not prove
all admitted inputs, hostile-input refusal order, arithmetic-bound sufficiency,
or correctness of every `Fraction` operation outside the finite rows.
