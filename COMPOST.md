# Dragon's Clutch compost policy

`~/dev/dragons-clutch` is a record of experiments, invariants, counterexamples,
measurements, and implementation techniques. It is not a dependency and will
not be merged into dClutch during the architectural restart.

## Allowed uses

- recover user intent and product requirements;
- cite an invariant, hostile case, measurement, or primary-source review;
- compare a fresh implementation against an old test vector;
- transplant a small, clearly owned pure kernel after provenance review; and
- preserve commit references for a later unrelated-history graft.

## Forbidden uses

- bulk-copy crates, adapters, action routers, account graphs, or generated DTOs;
- recreate the 46-slot foundation or cumulative action namespaces by inertia;
- retain old and new authorities in parallel "temporarily";
- import mock provider authority into a current profile; or
- copy any code with provenance outside Dragon's Clutch or dClutch without a
  separate explicit license/provenance decision.

## Transplant procedure

Before code crosses repositories, add a row below and a focused manifest under
`docs/compost/` containing:

1. the old repository commit and paths;
2. the written semantic invariant being retained;
3. provenance and license conclusion;
4. the new semantic owner and why it belongs there;
5. API and layout changes made during transplantation;
6. adversarial tests recreated from the invariant; and
7. old assumptions deliberately rejected.

Prefer implementing from the invariant without looking at the old function
body. Byte-for-byte copying requires a specific justification.

## Transplant ledger

| ID | Invariant | Source commit/path | New owner | Status |
| --- | --- | --- | --- | --- |
| — | No code transplanted at repository bootstrap | — | — | closed |

## Future history graft

When dClutch has a coherent independent implementation, the repositories may be
joined with preserved unrelated histories and dClutch made the current tree of
the Dragon's Clutch project. That operation will have its own reviewed plan. It
must not flatten, rewrite, or pretend that the restart was ordinary linear
development.
