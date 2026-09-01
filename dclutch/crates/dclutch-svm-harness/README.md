# dClutch SVM harness

This is a standalone, real-SVM integration harness. It deliberately loads
compiled successor role-program ELFs through `solana-program-test`; it never
registers a native processor or mocks an adapter.

Build the intended SBF artifacts first, then run from this crate with an
explicit artifact directory:

```sh
SBF_OUT_DIR=../../target/deploy cargo test --test resolution_core_v3_lifecycle
```

Each campaign fails early with an honest prerequisite message naming the ELF it
could not read.

Every campaign that loaded the gen-2 monolith `dclutch_sbf.so`, or one of the
gen-2 measurement programs, was banished with those programs on 2026-08-27; the
deleted files are listed in `~/dev/dclutch-legacy/svm-harness-tests/` and remain
in git history. The `RelayedMainnetStateV1` campaign was not banished: its
adapter moved into the Resolution role program, and it now executes against the
compiled Core and Resolution ELFs:

```sh
cargo build-sbf --manifest-path programs/dclutch-core-sbf/Cargo.toml \
  --sbf-out-dir target/deploy
cargo build-sbf --manifest-path programs/dclutch-resolution-proof-sbf/Cargo.toml \
  --sbf-out-dir target/deploy
SBF_OUT_DIR=../../target/deploy cargo test --test relayed_mainnet_state
```

Nineteen cases across three arcs. The **transport**:
create/append/seal/retire, the hostile corpus (five creation substitutions
named by refusal code, plus the signature, cluster, slot and replay corpus), a
below-threshold quorum, and the §4.10 swap tripwire. The **consumption**: a
sealed graduation resolving the market through the Product's own domain, plus
the corpus of signed-but-wrong pool bodies, foreign clocks, and observations
outside the window. The **liveness walk** (§4.8, §12.7): a market no relayer
answered for walked to a terminal `ResolutionFailure` and the walker paid the
manifest's own quoted bounty, with four refusals — before the deadline, twice,
against a live compartment that is not this walk's escrow, and against an
escrow one lamport short of what the market promised.

The signatures are cryptographically real; everything they attest is synthetic,
so this is neither devnet nor mainnet evidence.

The successor Resolution campaign executes the compiled Registry and
Resolution ELFs against the provenance-pinned local-validator projection of
the captured Pyth receiver/router programs and account bodies. It covers one
primary success and replay refusal, then funded recovery, funded exhaustion,
and explicit Product failure on one initially fresh Source. Each deterministic
certificate PDA is prepaid through a real System transfer with exact rent plus
tolerated dust and allocated/assigned by the compiled Resolution program. A
second fresh Source reaches exhaustion before an occupied final certificate
proves late transaction-wide rollback across Source, certificate, funding, and
worker state:

```sh
cargo build-sbf \
  --manifest-path programs/dclutch-registry-sbf/Cargo.toml \
  --lto --optimize-size --sbf-out-dir target/deploy
cargo build-sbf \
  --manifest-path programs/dclutch-resolution-proof-sbf/Cargo.toml \
  --lto --optimize-size --sbf-out-dir target/deploy
SBF_OUT_DIR=../../target/deploy \
  cargo test --test resolution_successor -- --nocapture
```

An exact `git archive b0e515f` build produced a 210,528-byte optimized
Resolution V3 ELF with SHA-256
`f684b845a60a25e661dee334e2866895d830956aedba74c8e1bf705d5abee2e7`
and an 89,760-byte Registry ELF with SHA-256
`b7d6634a23de84cb1b1f0a3368493b9008d88278c460f90e26b522af5e9a6e39`.
The clean ProgramTest campaign observed these instruction costs:

| Transition | Compute units |
| --- | ---: |
| Registry Resolution-role reauthentication | 128,793 |
| Primary Pyth success plus certificate creation | 242,546 |
| Primary replay refusal | 7,916 |
| Under-rent certificate refusal at final gate | 239,835 |
| Funded recovery plus certificate creation | 294,002 |
| Funded exhaustion plus certificate creation | 292,213 |
| Explicit Product failure plus certificate creation | 290,172 |
| Rollback lineage recovery plus certificate creation | 295,502 |
| Rollback lineage exhaustion plus certificate creation | 293,713 |
| Occupied failure certificate late refusal | 287,477 |

This is local real-SVM evidence. The captured update is synthetic-local, and
the campaign is not provider availability, devnet, deployment, or mainnet
evidence.

**THAT CAMPAIGN HAS NO INSTRUCTION BUILDERS, AND THE TABLE ABOVE IS
HISTORICAL.** Measured 2026-09-01 at HEAD with all eight ELFs freshly built,
by removing each layer of cover in turn and rerunning:

1. `#[ignore]` since `583e5bfa` (2026-08-26 01:06). The command above runs zero
   tests without `-- --ignored`.
2. With `--ignored`, six `DCLUTCH_*_SEMANTIC_RELEASE_ID` environment variables
   **nothing in this repository set** — no script, no CI row, no runbook. Now
   derived from the tree, with the operator override kept.
3. The pinned Resolution identity was `RESOLUTION_CONTROLLER_RELEASE_ID_V4`
   while the program authenticates V7. Corrected.
4. The activation cache address was derived and the account never seeded, so
   the first instruction refused `RegistryError::ActivationCache` (`0x1005`).
   Seeded, through the contract's own writer, as the lifecycle campaign does.
5. Past all four it stops at `resolution_successor.rs:1225`:
   **`primary_instruction` and `funded_caller_instruction` are `panic!` stubs.**
   `d1325c7f` replaced both at 2026-08-26 00:19, and the `#[ignore]` landed 47
   minutes later. The body of this campaign has been unreachable ever since,
   while this README went on quoting its compute table as evidence.

The disposition is convergence, not repair: rebuilding those two builders means
porting the current Core-effect and funded ABIs into a second fixture, and
`resolution_core_v3_lifecycle.rs` already carries them against the current Core,
Custody, Registry and Resolution ELFs. The funded walk belongs there. This
file's remaining value is its captured provider projection.

The consequence is specific and belongs to C-09: the Pyth **funded fallback
walk** — `FundedTransitionActionV3::{FailNext, Exhaust, CommitFailure}`, the
route a market takes when the provider goes silent — executes on a real ELF
*only here*. `resolution_core_v3_lifecycle.rs` reaches a failure-terminal
Market and closes its fund, but from a seeded `TerminalFailure` prestate; the
walk that produces that prestate is not driven. The remaining coverage is
`programs/dclutch-resolution-proof-sbf/src/tests.rs`, which calls
`process_instruction` in-process against hand-forged `AccountInfo`s and whose
funded cases exist to prove *removed* V1 dispatch stays removed.

The experimental Direct successor's four-ELF physical campaign is GONE, with the
three first-party ELFs it drove. `physical_direct_composition.rs`,
`claims_proof_target.rs` and `registered_claims_proof_target.rs` were banished
to ~/dev/dclutch-legacy/svm-harness-tests/ together with
`programs/dclutch-{claims,custody,controller}-proof-sbf`: they were the DCLTCAT1
stratum's proof artifacts, and the Market representation they composed has no
writer in this tree. The successor role programs carry their own campaigns.
The lookup-table, rollback and token observations those campaigns made are not
re-derived anywhere; they were evidence about programs that no longer exist.
