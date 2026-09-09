# Resolution reclaim retains the submitted Registry pin — 2026-09-09

## Decision and native correction

A mutable Registry with its exact admitted deployment slot and retained authority
is valid. Decision [0012](../decisions/0012-devnet-iteration-substrate.md) establishes
that policy. `crates/dclutch-registry/src/immutable_registry.rs` implements it in
`authenticate_pinned_registry_release_v1`; its
`pinned_registry_release_admits_a_mutable_substrate_until_it_moves` test is the
canonical control. `tools/local-validator/bootstrap/successor/src/local_mutable.rs`
intentionally retains the checked cohort authority, including Registry.

Resolution Submit authenticates the Registry artifact selected by the Core-owned
infrastructure profile (`provider_transport_v3.rs::authenticate_infrastructure`).
Execute uses the same pinned-artifact policy in
`provider_instruction_v3.rs::authenticate_market_and_infrastructure`. Reclaim and
Abandon instead shared `authenticate_reclaim_release`, which required Registry
ProgramData authority to be absent. Their lifecycle retained only the Registry
program identity; their frame supplied no artifact evidence. Removing that guard
alone would allow an upgraded Registry to provide replacement activation records.

The successor `ProviderUpdateLifecycleV4` retains the authenticated Registry
artifact digest in the same Resolution-owned lifecycle account. Its exact width
is 560 bytes, magic is `DCLTPUL4`, version is 4, and its PDA domain is
`dclutch/provider-life/v4`. The prior 528-byte layout and old magic refuse; there
is no active legacy decoder. Transport request formats remain V3. Submit pins the
hash of its already-authenticated, read-only artifact account. Both Reclaim and
Abandon append its raw record and vacant staging cursor at indices 18 and 19.
They authenticate that finalized record against the lifecycle digest, then call
the canonical pinned Registry authentication with the shared live Loader observer.

This preserves artifact identity, owners, executable flags, ProgramData link,
deployment slot and exact authority. The lifecycle is the replay authority for
this pin; it does not depend on a still-existing Market or a later infrastructure
selection. Rust consumers, operator construction, browser WASM, and its generated
SDK facts move together. Browser Submit poststate verification binds the stored
artifact digest to the original Submit artifact address. Direct's host projection
hashes durable Submit pre-account 10 and checks Reclaim's two derived coordinates.

## Controls and boundaries

Tests ran in the warm, isolated workspace target at
`/tank/dregg-build/dclutch-claims-admit-cu-v2-20260909/source`, through `swarm-build`.
The source checkpoint printed before the native run was
`fc0de4476fc5d8d6da7fa79b9641538aef31ca7b`, with the coordinated changes carried by
this evidence's commit. No extra all-program SBF build was run by this lane.

`reclaim_registry_tests` runs an 18-case matrix: Submitted and Consumed lifecycle
states each receive an accepted mutable Registry, later slot, earlier slot,
changed authority, changed artifact, substituted staging address, wrong Program
owner, wrong ProgramData owner, and wrong ProgramData link. Accepted deployment
reaches an intentionally empty downstream ActivationCache sentinel. Later slot
returns `ResolutionError::ReleaseSuperseded`; earlier slot, authority, owner and
link controls return `ResolutionError::ResolutionDeployment`; artifact and staging
controls return `ResolutionError::FinalizedRecord`. A separate accepted helper
control returns success.

Restoring the old immutable guard makes both tests fail, including the accepted
mutable deployment. Removing only the new Registry authentication call makes all
hostiles reach the downstream sentinel and fails the matrix. Restoring the gate
passes both tests. These are native shared-admission-boundary tests; they do not
claim a completed Reclaim CPI or actual SBF Reclaim/Abandon execution.

Retained logs and replay script, relative to
`/tank/dregg-build/dclutch-claims-admit-cu-v2-20260909/continuity/`:

- `reclaim-immutable-red.log`: 0 passed, 2 failed.
- `reclaim-omitted-pin-red.log`: 1 passed, 1 failed.
- `reclaim-restored-green.log`: 2 passed, 0 failed.
- `reclaim-red.sh` and its accepted source/old-guard inputs.
- `reclaim-host-tests.log`: lifecycle successor codec, 20-account inline Reclaim
  packet, and browser-owner Submit poststate tests each passed individually.

Focused local checks of Resolution, provider transport operator and Source-provider
WASM passed; `cargo metadata --locked` and the generated Source-provider WASM
verification passed. Direct separately passed its focused finalized Submit,
finalized Reclaim, and stable account-union controls. Browser Vitest was attempted
but did not start because `vitest/config` was not installed in the canonical app;
this is not a browser-test pass. Generated WASM was rebuilt by the owner generator.

## Existing bank remains blocked

The old `691bc6ff0` bank's actual Reclaim simulation refused
`ResolutionError::ResolutionDeployment` at 15,991 CU. No transaction landed and
neither its Registry authority nor its program image was changed. Evidence is
`/tank/dregg-build/dclutch-direct-continuation-4bee7cc3a-20260909/continuation-run1/reclaim-refusal-691bc6ff0.json`,
SHA-256 `bacc9feb0a9de3c5ae9412a746fb28c087aed93878a79e01efd2ce7d541894dd`.
Fresh cohort execution of the committed successor is a separate deliverable.
The combined frame ratchet remains owed to the root's inclusive SBF build.

## 2026-09-09 addendum — direct native/WASM parity

After commit `a28a1662c7602d31e523cf22e825c13a8b8b29e1`, the native
`source-provider-parity` binary ran through the same hbox workspace target and
produced Reclaim, Submit-fresh, and Submit-poststate fixtures. Node loaded the
committed generated WASM directly and compared each complete output byte-for-byte
with the native output: all three matched. Reclaim has 20 accounts, two required
signers, and an inline packet of 1,152 bytes. The canonical app Vitest startup
limitation remains; this direct parity check does not relabel it a Vitest pass.
The three JSON fixtures and `reclaim-wasm-parity.log` are retained in the
`continuity/` directory named above.
