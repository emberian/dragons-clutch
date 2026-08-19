# Typed artifact transport

Status: **implemented bring-up path with real-SBF bank evidence; not audited or
deployment-authorized**.

Dragon's Clutch has immutable artifacts that are larger than one instruction
can reasonably carry.  In particular, a Terms account is 1,656 bytes.  The
transport in `clutch-solana-layout::artifact` and
`clutch-sbf::instructions::artifact` admits those bytes without introducing a
generic blob account or a privileged uploader.

## Lifecycle

The transport recognizes exactly three artifact kinds:

| kind | exact body | final identity |
| --- | ---: | --- |
| collateral policy | 266 bytes | `policy(Profile, policy digest)` |
| price grid | frozen `PriceGridAccount` width | `grid(Realm, grid digest)` |
| terms | frozen `TermsAccount` width | `terms(Realm, terms digest)` |

There is deliberately no caller-defined artifact kind or length.

1. `BeginArtifact` creates an exact-size uploader-keyed stage PDA.  Its header
   freezes the artifact kind, context, digest, exact length, funder, creation
   slot, expiry slot, cursor, and canonical bump.
2. `WriteArtifact` accepts only the unique next 192-byte chunk.  The final
   chunk may be shorter; unused wire bytes must be zero.
3. `SealArtifact` requires a complete live stage, validates the body through
   the artifact's owning hostile-byte codec, authenticates the final PDA, and
   copies the exact raw bytes.  Consumers never accept a stage as an artifact.
4. The funder may `AbortArtifact` at any time.  After expiry, any signer may
   reap it.  In both cases every stage lamport returns to the funder persisted
   at Begin; the reaper receives no bounty and neither collateral nor Hoard
   principal is a rent source.

Sealing an artifact that already exists is idempotent only when the existing
program-owned account has the exact width, decodes under the same binding, and
is byte-identical.  Otherwise it refuses.  A stage is closed only in the same
atomic successful transaction that admits the final artifact.

## Fail-closed properties

- Stages are uploader-scoped, so one abandoned partial upload cannot occupy
  the unique final content-derived address or another uploader's stage.
- The header rebinds every PDA seed on every write, seal, abort, and reap.
- Gaps, overlaps, duplicate chunks, post-completion writes, mixed bindings,
  nonzero chunk padding, and invented lengths refuse before state changes.
- Unwritten stage bytes must remain zero, including in hostile genesis images.
- Writes and seals at slots after the frozen expiry refuse.  Public reap is
  admitted only strictly after expiry, so the boundary has no overlap.
- A final policy, grid, or Terms body is not trusted because it hashes to the
  requested address: it must also pass the owning semantic decoder.
- All instructions use sequence zero because the stage cursor and lifecycle
  are the replay state.  A nonzero envelope sequence refuses rather than
  creating a second replay truth.

## Runtime SHA boundary

The portable layout crate is dependency-free and recomputes SHA-256 in safe,
fixed-array Rust.  That remains the host oracle.  Interpreting its 1,620-byte
Terms preimage as SBF instructions exceeded the default 200,000-CU transaction
budget.

On `target_os = "solana"`, the adapter therefore:

1. runs `TermsAccount::decode_unchecked_into`, whose documented omission is
   only digest recomputation and which retains every other hostile-byte and
   semantic check;
2. hashes the exact canonical preimage
   `"dragons-clutch/terms/v2" || raw_terms_body` with the safe
   `solana-sha256-hasher` wrapper over Solana's native SHA-256 syscall; and
3. compares that result to both the staged binding and the digest stored in
   the decoded Terms account.

Host builds continue through the portable full decoder.  Thus the optimized
path changes the hashing implementation, not the domain, bytes, or admission
relation.  The syscall and SVM runtime remain an explicitly unverified adapter
boundary.

## Evidence

`programs/clutch-sbf/svm-tests/tests/artifact_transport.rs` drives the real SBF
ELF in `solana-program-test`.  Its restart case:

- creates an absent Terms stage through System Program CPI;
- commits three chunks;
- proves an early seal refuses and rolls the stage back byte-exactly;
- reloads the uploader and stage account images into a fresh bank;
- proves a duplicate historical chunk refuses and rolls back byte-exactly;
- appends the remaining chunks;
- creates the final content-derived Terms PDA; and
- observes exact raw Terms bytes and stage closure;
- repeats the upload against the already-present final, proving the exact
  idempotent path returns all second-stage rent; and
- submits a structurally valid Terms body with a stale digest, proving the
  native-SHA path refuses atomically and creates no final account.

A second bank case uploads and seals both other admitted artifact kinds and
observes the exact canonical collateral-policy and price-grid bytes at their
respective final PDAs under the default transaction budget.

The expiry case proves an unrelated signer cannot abort a live stage, then
warps beyond expiry and proves that signer can reap it while all rent returns
to the recorded funder and none goes to the reaper.

In the local real-SBF run on 2026-08-18, the largest first write consumed
28,751 CU and a new Terms seal consumed 14,597 CU under the default 200,000-CU
budget.  These figures are execution evidence for that build, not a frozen
cost promise.

The whole program's current SBF build still reports pre-existing stack-frame
warnings in unrelated reference/order-batch functions.  Consequently this
bank campaign is evidence for the artifact transactions it executes, not a
release attestation for the entire ELF.

## Deliberate limits

- The only transported kinds are policy, grid, and Terms.  Source
  specifications, archive pages, candidate feeds, and clearing artifacts need
  an owning codec and an explicit new kind before they can use this path.
- Expiry is a slot bound, not a wall-clock promise.
- Program upgrade authority and loader provenance are outside this transport.
- The bank restart is account-image rehydration, not validator-ledger replay.
- No public RPC, cluster, wallet, deployment, signing service, or funds are
  used by this evidence.
