# Cargo target isolation — 2026-09-08

Owner: CODEX-INTEGRATION. This is build-instrument evidence, not protocol execution.

A clean checkout does not establish source provenance if it shares Cargo's target
with a different source checkout. At live revision
`9e87f291c250bc1061d4cc0fae8dceed9faccfd6`, the integration checkout at
`/private/tmp/dclutch-cut-check-c3f1a152b` contained the committed three-argument
Prepare emitter and its three-argument caller. Its filtered bootstrap test,
using `/Users/ember/dev/dclutch/target`, nevertheless compiled against metadata
for the live dirty two-argument emitter and failed with E0061. `cargo metadata`
resolved both package paths to the clean checkout; that alone did not detect the
foreign compiled dependency. Log: `/private/tmp/dclutch-cut-9e87-general-control.log`.

## Independent control

A minimal two-checkout probe reproduced both silent success and the false type
error with the repository's Cargo 1.97.1. Both directories contain a package
`isolation-probe` depending by relative path on `isolation-dep`, version 0.1.0,
edition 2024. They have no external dependencies. In A, `f` takes three `u8`
arguments and the caller passes three zeros. In B, both definition and caller
use two. Both source trees are created before the first build.

| Invocation | Target | Actual result |
| --- | --- | --- |
| Check A | shared | Both packages compile; exit 0 |
| Check B | same shared | No package compiles; exit 0 |
| Append a comment to B's caller and check B | same shared | E0061 claims B's function takes three arguments; exit 101 |
| Check that unchanged B | fresh isolated | Both packages compile; exit 0 |

Every command was `rustup run 1.97.1 cargo check --offline --target-dir TARGET`
from the named source directory. The failed check's diagnostic points to the
relative dependency source while describing the other checkout's signature.
The probe and source trees remain in
`/private/tmp/dclutch-cargo-isolation-yva3gwxw`; exact output is retained in
[cargo-cross-checkout-2026-09-08/pinned-results.json](cargo-cross-checkout-2026-09-08/pinned-results.json).

## Consequence

A source checkpoint uses its own workspace-root target. Do not share a
`CARGO_TARGET_DIR` across live and detached checkouts. This preserves the single
workspace/lock/target rule within each checkout; it does not introduce per-crate
workspaces. Heavy checks remain on hbox through `swarm-build`.

The earlier shared-target integration checks are not accepted as independent
proof of the committed dependency closure. A fresh isolated local attempt then
failed with ENOSPC before the selected test ran; its log remains at
`/private/tmp/dclutch-cut-9e87-general-isolated.log`. Only that failed temporary
build target was removed, recovering space without deleting shared workspace
artifacts. The General owner is rerunning the exact committed-source host
control on hbox. No publication was made on the strength of these checks.
