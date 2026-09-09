# General V5 durable browser operation — 2026-09-09

The existing `/general` page now offers a Wallet Standard execution path for
all retained native V5 plan actions. `dclutch general plan` remains the existing
read-only producer. This change does not introduce a TypeScript request or
transition compiler, and does not establish acceptance of General's full
runtime lifecycle under the production compute limit.

## Implemented boundary

The browser binds the imported Trading program to the selected deployment before
a wallet can be prompted. Asynchronous selection changes stop the wallet/send
continuation. The browser first reacquires the native plan's dependencies and lifecycle
prestate. It compares the root bytes with the imported root-prestate digest,
then simulates the unchanged packet with its original blockhash and requested
writable accounts. The bounded shared RPC method admits at most 32 distinct
poststate addresses and a 1232-byte packet; it never replaces the blockhash.
Its execution error, compute consumption and return data remain observations,
not evidence of landing. General requires a successful native simulation and
an exact Trading HotAck/root-poststate join before offering signing.

The review shows account addresses, lamport movements, Claims balances, and
admitted Token-2022 amounts, mint and owner through existing canonical SDK
decoders. The simulation inputs are reacquired after simulation to refuse
concurrent changes. Native plan text and the review are persisted before any
wallet prompt. Each wallet adds only its own signature to the same message;
the reviewed writable inputs are checked again first. On reload, an unsigned
operation restores the exact message; partial signatures held only by the
previous page must be collected again.

The complete signed packet and its first signature are persisted before the
send. An interrupted send leaves that exact packet available. Recovery never
changes the blockhash or reconstructs a different transaction. Resending is a
separate explicit button and sends the same retained bytes with no RPC retry
loop; polling is offered separately.

Finalization requires the RPC's exact signed transaction bytes, successful
execution, Trading return-data producer, the canonical HotAck joins, and the
current root bytes hashing to the receipt's root-poststate digest. A root
which has advanced beyond this receipt is refused, and the journal remains.
Lifecycle accounts are separately reacquired and canonical decoded. Those
later snapshots are explicitly not advertised as independent economic
reconciliation. Submitted journals remain retained on receipt acceptance. A separate user
action rechecks finalization and root bytes, persists the complete journal and
receipt in an archive, and only then releases the active operation slot. This
allows the next native action while preserving the previous signed evidence. The shared finalized-refusal archive
also permits a rejected packet to be archived and replaced only after exact
finalized failed signature, slot, and wire checks.

## Validation

Validation used existing frontend dependencies in
`/private/tmp/dclutch-featured-standing-validation` (base revision
`a7e6d43000f24f2ed58d36c69082fc48e099fa8e`), copying only this lane's named paths
and preserving other lanes' files. No browser wallet, live RPC, signature,
submission, validator or new native build was used.

- Shared RPC simulation: four focused tests pass, including exact packet and
  flags, refusal preservation, count/floor/status refusal, and pre-network bounds.
- General finalized observation: three focused tests pass. Changed packet,
  producer, request commitment and root bytes each hit the named refusal;
  unavailable finalization remains distinct from a finalized compute refusal.
- General durable operation: eight focused tests pass. They cover recovery
  before wallet access, persistence and stale-input checks before prompting,
  exact wallet signing and message-rewrite refusal, interruption after durable
  signed persistence but before successful send, exact-byte resume, and altered
  plan/preview refusal, selected-program and asynchronous-selection refusals, and
  archive storage failure before active-journal release.
- Removing only the pre-wallet stale-input check made its targeted test fail:
  the wallet was reached instead of `General reviewed input changed`. Restoring
  it returned the targeted suite to green.
- Removing only the finalized root-hash comparison made the changed-root
  targeted test resolve instead of refuse. Restoring it returned the suite to
  green.
- The existing all-action General plan controls pass 17/17. Scoped SDK/web
  TypeScript checks and lint pass. The final full-web check is separately blocked
  by the Dealer lane's pending native generated module/export integration; no
  General diagnostic occurred in that check.

The operation test packet is explicitly derived from the archived native CLI
fixture with a test signer, current generated heap declaration, and artificial
root bytes. The archive is unchanged. These tests exercise browser integrity
boundaries; they are not a new native execution or cohort fixture.

## Remaining production entrance work

The user currently supplies an existing native CLI plan. General has no
production browser native-intent/WASM constructor or independent child
poststate verifier. Adding those should reuse `general_successor` production
construction, not reproduce its byte or economic semantics in TypeScript.
Matched nonzero General execution and settlement remain the runtime lane's
work; this browser change does not claim their acceptance.
