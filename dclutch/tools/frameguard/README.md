# Frameguard

`run.sh` freshly builds all 13 program links with LLVM stack-size sections and
compares every canonical function frame with `baseline.json`. It catches frame
growth below SBPF v0's 4,096-byte hard wall; the ordinary build diagnostic does
not.

```sh
tools/ci/run.sh frameguard
tools/ci/run.sh --commit HEAD frameguard  # quoteable, archived source
```

Exit 0 is agreement, 1 is a build/diagnostic/frame disagreement, and 2 means a
prerequisite or measurement was absent. The build must freshly compile each top
package and emit zero stack-overwrite diagnostics before comparison.

The baseline is an exact ratchet, not a ceiling. Shrinkage is red until the
smaller manifest is admitted, so recovered headroom cannot be spent again.
Capture twice into scratch paths, then accept only the identical pair:

```sh
tools/frameguard/run.sh --capture /tmp/frame-a.json
tools/frameguard/run.sh --capture /tmp/frame-b.json
tools/frameguard/frameguard.py accept \
  --first /tmp/frame-a.json --second /tmp/frame-b.json \
  --output tools/frameguard/baseline.json
```

Read the complete baseline diff before committing it. A new function, removed
function, changed instance count, growth, and shrinkage all require that review.
