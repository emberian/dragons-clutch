# Direct AOT contract

This crate is a safe, `no_std`, `no_alloc` stateless implementation of the one
Lean-owned Direct ordinary-matching `ProgramV2`. The generated module contains
the exact 856-byte program and register indices emitted from
`DClutchSemantics.DirectProgramV2`; the handwritten evaluator implements that
same descriptor meaning over borrowed runtime banks.

The accelerator is not an effect authority. Trading must execute the canonical
interpreter and this AOT implementation over the same authenticated input,
require equal acceptance/refusal and accepted output bytes, project one common
effect plan, and commit once. AOT-only execution remains unavailable until a
Registry-owned descriptor/certificate/artifact admission or checked proof route
exists. The descriptor meaning does not change when such a route is added.

The 41-scalar/four-identity bank is this Direct descriptor's semantic shape. It
does not cap Product outcome width: the descriptor names one authenticated
outcome and its runtime domain width rather than embedding an `N`-sized vector.

`./check-generated.sh` builds the Lean owner, emits only to a temporary file,
validates the measured shape, formats it, and compares it with `src/generated.rs`.
It never redirects a failed generator into the canonical source file.
