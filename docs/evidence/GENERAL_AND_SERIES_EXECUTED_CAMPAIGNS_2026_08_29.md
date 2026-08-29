# General and Series executed campaigns — 2026-08-29

Evidence class: **executed on a local `solana-test-validator`** against real SBF
ELFs, at commit `920df2bdb66ea328852283e21e4d2c68733bac6c`. Every number below
was measured from a finalized transaction on a private localhost ledger, not
from `ProgramTest` and not from simulation. No devnet, no mainnet, no wallet,
and no deployed release identity is claimed. The ledgers were discarded after
measurement; the campaigns are reproducible from the recipes here, and one was
re-run clean-room to prove exactly that.

## Result

Two families that had artifacts and no caller now have callers that ran.

1. **General** — all seven authored actions **accepted** against the real
   accelerator ELF, at runtime widths 1 and 4. Eleven finalized transactions
   per run, because the settlement half is a chain rather than seven shots.
2. **Series** — the **first executed Series Found in this tree's history**: one
   finalized `series_consume`, plus an on-chain-committed hostile.

Neither reaches Trading's commit half. That is one shared gap, not two, and it
is recorded as an open protocol decision in `WAVE.md`.

## General: seven actions, eleven steps, all accepted

`local-private-validator-general-hot-campaign-v1` in
`tools/local-validator/bootstrap/successor`.

Seven authored actions occupy **eleven steps**: `Collect` and `Distribute` each
consume the three settlement manifest rows, and `Materialize` sits between them
because the collected inventory must exist as a complete set before anything is
distributed. Every settlement step reads the cursor the previous one produced,
and the whole chain is derived natively — in `general_settlement_fixture.rs` —
before a single transaction is signed, so a broken chain fails on the host.

| step | action | ACK | CU | legacy packet |
|---|---|---|---:|---:|
| 0 | Consider | accepted | 34,825 | 863 |
| 1 | Freeze | accepted | 31,371 | 797 |
| 2 | InitializeSettlement | accepted | 60,466 | 920 |
| 3–5 | Collect × 3 rows | accepted | 55,703 / 56,908 / 56,885 | 900 |
| 6 | Materialize | accepted | 51,884 | 866 |
| 7–9 | Distribute × 3 rows | accepted | 55,654 / 56,859 / 56,890 | 900 |
| 10 | Close | accepted | 60,046 | 885 |

Also 11/11 accepted at `--outcome-count 4` (CU 31,516–61,379). Packets do not
move with the width because the register bank travels in scratch pages, so what
the 1,232-byte ceiling binds is the **account frame**, not the bank.

**The width is not a preference.** Six of the seven serialise to 1,273–1,329
bytes at N=258 against the 1,232-byte ceiling, so a campaign claiming that width
would record routes no validator would accept. The caller refuses N=258
explicitly rather than discovering it.

**Nothing in the settlement frame is a literal.** The verifier cursor, the
verified-candidate certificate and the manifests are outputs of a real
collection half: a batch opened on a live root, three signed portfolio orders
admitted, the batch closed, a candidate addressed by its own digest, a
submission funded, and every row through `verify_candidate_row_v1`.

### Recipe

Build the two ELFs, then run two phases — the first opens no socket and no key,
because its output is the genesis the validator must start from:

```sh
cargo build-sbf --manifest-path programs/dclutch-general-accelerator-sbf/Cargo.toml --sbf-out-dir "$SBF"
cargo build-sbf --manifest-path programs/dclutch-general-accelerator-sbf/test-programs/general-caller/Cargo.toml --sbf-out-dir "$SBF"

# phase 1 — emit 95 genesis accounts and 11 planned journals
dclutch-local-successor-bootstrap local-private-validator-general-hot-campaign-v1 \
  --accelerator "$ACC" --caller "$CALLER" \
  --account-dir "$D/accounts" --journal-dir "$D/journals" --evidence "$D/evidence.json"

solana-test-validator --ledger "$L" --reset --rpc-port 21400 \
  --faucet-port 21402 --gossip-port 21403 --dynamic-port-range 21410-21441 \
  --ticks-per-slot 16 --account-dir "$D/accounts" \
  --bpf-program "$ACC" "$SBF/dclutch_general_accelerator_sbf.so" \
  --bpf-program "$CALLER" "$SBF/dclutch_general_accelerator_test_caller_sbf.so"

# phase 2 — sign, submit, finalize
... --execute --rpc-url http://127.0.0.1:21400 --payer-keypair "$D/payer.json"
```

### The one thing to carry forward

**Five of the General Hot bank identities are route-dependent, and for
`SOURCE_VAULT_CONTEXT` and `DESTINATION_VAULT_CONTEXT` zero is what the
*enabled* route requires.** An unwritten register does not read as absent; it
reads as live. That asymmetry is why a partly-filled bank looks like it is
almost working — the actions whose zero defaults happen to be correct pass and
the rest refuse — and it cost three validator rounds to find. Write all of them
explicitly.

## Series: the first executed Found

`local-private-validator-series-consume-v1`. `series_consume` is the only Series
route this tree dispatches, and it had only ever run inside `ProgramTest`.

| | |
|---|---:|
| outcomes | 258 |
| finalized slot | 60 |
| compute units | 624,620 |
| account metas | 62 (61 unique) |
| instruction data | 656 bytes |
| routed wire | 1,037 bytes (v0 + ALT) |

Signature
`3Qpa7WSKag8nFQsm8vMcrFbaRqe3M27ajfs1QVAvHgTQ3GVWm3ZbLea67gAFyPRjRZbnb7V2uiEt8Rn9mDanb9F4`.
Measured draw is **below** the 722,142 CU recorded in `docs/reference/budgets.md`.

**A write path's acknowledgment is the state it committed**, not a returned
buffer. The consumer authenticates the Market being Core-owned and written and
the founding permit holding exactly 5,122,560 lamports — the two facts the
ProgramTest asserts. The journal ladder is planned → prepared → submitted →
finalized, each phase renamed into place; rerunning a finished campaign is a
no-op, and that check necessarily *precedes* the vacant-Market preflight,
because a succeeded Found is exactly what makes the Market non-vacant.

### Why this was hours and not days

**The ~1,250-line fixture was not ported.** It stays in
`programs/dclutch-core-sbf/tests/found_program_test.rs` with its one author and
gained a single `#[ignore]`-gated emitter,
`emit_series_consume_validator_campaign`, which builds the campaign exactly as
every other Series test does, starts the genesis it would have run against, and
reads every account the instruction names back out of the banks client. Output:
61 genesis account JSONs (2 absent by design — `funding_source` and
`funding_source_replay` are deliberately never created) plus a manifest.

Reading rather than reconstructing is what made four hazards dissolve instead of
needing solutions:

- **exact lamports** — `series_consume` compares `market.lamports()` to
  `request.market_rent()` with `!=`, so a rent heuristic silently refuses. The
  value is observed, never recomputed.
- **six loader-v3 Program/ProgramData pairs** — their deployment slot flows into
  the release-set digest and therefore into the Market PDA, making
  deploy-then-derive circular with genesis. They are read back with their real
  bytes and executable bits and written as genesis JSON. **No `--bpf-program`
  flag is used at all.**
- **compute budget** — `bounded_instructions` already prepends
  `set_compute_unit_limit(1,400,000)` and refuses duplicates; a validator's
  200,000 default would refuse.
- **the ALT** — `LookupTableMeta::default()` is `deactivation_slot: u64::MAX`
  and `last_extended_slot: 0`, i.e. already active and extended before any live
  slot, so genesis injection just works.

The fifth, the occurrence's 10,000-slot retry window measured from slot zero, is
bought off by running at the **default** tick rate: ~66 minutes of headroom
instead of ~16.

Legacy routing is not tight here, it is impossible: 61 unique keys is 1,952
bytes of addresses against a 1,232-byte packet.

### Hostile, committed rather than simulated

`--expect-refusal 12293` skips the vacant-Market precondition and requires the
transaction to fail with exactly that code. It **skips preflight on purpose**: a
hostile rejected by simulation proves what a simulator thinks, not what the
chain did. The double-consume replay committed in slot 535 carrying
`{"Custom":12293}` (`CoreSbfError::Market`) and left the Found byte-unchanged —
the permit still at 5,122,560 lamports. A hostile that *succeeds* is reported as
the loudest possible failure of the property it defends.

### Reproducibility

Re-run clean-room — fresh emit, fresh ledger, fresh port — yields the **same**
Market, permit, permit balance, 624,620 CU and 1,037 wire bytes. Only the
signature and slot differ.

## `release_v4`: the first assembler, and what it refuses to default

`programs/dclutch-trading-sbf/src/series/release_v4.rs`.

Series had complete V4 artifact *encoders* and not one production caller.
Nothing assembled them into a descriptor, so
`authenticate_series_consume_artifacts_v4` — the function deciding whether a
Series release is admissible at all — had zero callers of any kind, and the only
bundle in the tree was a unit-test descriptor built from placeholder identities
(`byte_id(10)`, `byte_id(11)`, …) whose digests deliberately match no real
artifact.

This emits the three fully-determined artifacts (account profile, request
profile, transition) and builds a descriptor naming every artifact by the digest
of bytes the caller is holding. That is the content of "self-consistent" and the
precondition for a Market ever selecting the release.

**Two artifacts are typed parameters, not defaults, and that is the point.**

- **lifecycle** — Series declares no `StateLifecyclePolicyV5`. Writing one
  decides which created states it covers, which rent-quote generation it pins,
  and who receives the refund — where `series/lifecycle.rs:149`
  `ticket_capability_refund` already suggests the Ticket's capability rent is
  spoken for by the funding path, so a policy also claiming it would be a second
  author for one lamport flow. General's analogue involves recipe seeds and PDA
  derivation; getting those subtly wrong yields a policy that **authenticates
  but derives wrong addresses**, which is worse than none.
- **strategy** — the ShadowAot arm names the *deployed* accelerator's
  certificate program. That is a fact about a deployment; a builder that
  invented one would address a release to an accelerator nobody runs.

The requirement the lifecycle must meet is derived off the verifier, in order,
as `SERIES_CONSUME_LIFECYCLE_REQUIREMENTS_V4` — so a future author can check an
answer against the verifier rather than against prose. The fourth conjunct is
why this blocks: `action_plan_count(Consume)` must be nonzero, so a policy
covering only Prepare or Expire decodes, validates against the profile, and is
still refused.

## What is not claimed

- Neither family reaches Trading's commit half. `hot_v3` is family-neutral and
  **no dispatch arm is missing**; what is missing is a published and selected
  capability release, which General lacks identically. See the two open protocol
  decisions in `WAVE.md`.
- Series `prepare` and `expire` have no release and have never executed
  anywhere. `series_open` and `series_permit_expiry` are dispatched in
  `dclutch-core-sbf` but have no SBF test; `tools/gauntlet/tier4/README.md`
  records them as never-executed.
- No devnet or mainnet statement. Every measurement is a private localhost
  ledger.
- The Series campaign is authored once but lives in two places (the ProgramTest
  emitter and the successor consumer). Closing that means **moving** the
  fixture, not copying it — a copy would give one campaign two authors that can
  silently disagree.
