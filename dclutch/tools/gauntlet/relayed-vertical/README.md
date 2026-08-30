# DEMO-VERT — the relayed graduation market, end to end, on a local rehearsal

> **Un-parked.** The two-phase order the old park banner prescribed is now the
> code: prepare the checked mutable substrate, boot a validator over the
> prepared account directory, run the administration campaign through
> activation, call `DirectMarketCompilerOwnedV1::load_local` against the LIVE
> deployment, compile the relayed market, and only then found and relay.
>
> **The shell runner below is stale and does not drive this.** It still passes
> `--spec-template` and never passes `--rpc-port`, so it dies with
> `--rpc-port is required` after building every ELF. The campaign binary's real
> contract is a checked release gate and the three digests that bind it:
>
> ```sh
> dclutch-relayed-vertical-campaign run --walk failure \
>     --transcript ABS.json --relayer-bin ABS/dclutch-relayer --work ABS/dir \
>     --rpc-port PORT --checked-release-gate ABS/CHECKED_UPGRADE_GATE.json \
>     --expected-gate-sha256 HEX64 --expected-source-revision HEX40 \
>     --expected-source-tree-sha256 HEX64 --seed HEX64
> ```
>
> Build the gate with `tools/release/checked-release-candidate.sh --work DIR`;
> it prints the gate digest, and the gate JSON carries `source_revision` and
> `source_tree_sha256`. Teaching the shell runner this contract is open work.

One journey-shaped campaign for the `RelayedMainnetStateV1` family
(`docs/design/MAINNET_STATE_RELAY.md`): a zero-cut graduation Product founded
with **no recovery policy**, resolved end to end on a local devnet rehearsal —
and its sibling, where the relayer goes silent and the funded deadline walk
pays a walker.

```sh
tools/gauntlet/relayed-vertical/run-relayed-vertical.sh --walk both
```

## The shape: two validators, one daemon, one market

- **The mainnet twin** — a stock `solana-test-validator` the campaign starts on
  its own port block, carrying the synthetic-of-real Meteora DBC world: the
  REAL mainnet Program/ProgramData addresses and the REAL 424-byte
  `VirtualPool` layout (§10.1), with a SYNTHETIC ELF tail, deployment slot,
  upgrade authority and pool. The CS dossier never captured those four values
  and this lane makes no public reads; the transcript enumerates which half is
  which, and every daemon artifact carries the rehearsal-twin label.
- **The successor validator** — the tier-1 producer's own founding campaign
  (`found_through_open`, compiled in by `#[path]` exactly as the journey does),
  in **transaction-only record publication**, founding the §12.8 record set:
  the producer publishes the source graph the material names, and this
  campaign publishes the two records the provider release names —
  `RelayerKeySetV1` (n=1, m=1 over the daemon's disclosed key) and
  `RelayedAdapterConfigV1`.
- **The daemon** — the real `dclutch-relayer` binary, driven across the same
  process boundary an operator would use: a TOML config, `run --dry-run` to
  observe and sign, then `submit-artifacts` to re-submit the exact recorded
  bytes after the keeper created the slot-seeded record. Re-sign, never
  re-observe.

## The success walk

found → CreateFund/VerifyFundReady over the **short no-recovery frame** (the
`e5b6923` admission's first live execution) → daemon observes the twin →
keeper creates the record for the observed slot → daemon appends ×4 and seals
(the full-body VirtualPool append rides the Market's address lookup table —
one of the family's two known over-packet wires) → the 28-account consumption
as a packet-safe v0 transaction → the Source state single-writes to Resolved,
the record is Consumed, and the `ResolutionSuccess` certificate selects the
zero-cut domain's one ordinary region.

## The failure sibling

The same market, founded with a short terminal window; no record is ever
created. Past `window.end + max_age` by the devnet clock, a fresh walker
submits `CommitDeadlineFailure` on a **bare legacy transaction** (measured
against the 1,232-byte limit — the one route that must work when nobody
cooperated never depends on a lookup table) and is credited exactly the
manifest's disclosed bounty from the market's own escrow. §12.7's sentence,
executed on a live validator: a silent relayer cannot make the market
unresolvable.

## The disclosed conflation

For this v1 graduation market, "the relayer went silent", "the venue was
upgraded" and "it never graduated" all land on the same pre-disclosed failure
outcome (§12.6). The sentence is compiled into the founding input and carried
verbatim in the transcript; the witness set refuses a transcript that stops
saying it.

## M-44 and M-45, answered where they are enforced

- **TransferHookPool blindness (M-44)**: after DBC 0.2.0 a `TransferHookPool`
  shares the identical 424-byte body. The Lean-authored decoding rules name
  BOTH discriminators, and `read_dbc_graduation` refuses the transfer-hook
  discriminator **explicitly** (`VenueDiscriminatorMismatch`,
  `crates/dclutch-relay-contract/src/decode.rs`) — a decision, not an
  accident, executed in the wire crate's own corpus. Admitting transfer-hook
  pools would be a new decoding-rules row, i.e. a release decision, not a bug
  fix.
- **The complete-flag latch (M-45)**: DBC's `complete` flag's irreversibility
  is unverified, so the consumer must never un-graduate. It cannot, by
  composition: a not-complete body is **no answer** (`WindowNotSatisfied`,
  never a negative resolution), and the first admissible observation
  terminalizes through a single-write
  (`resolve_primary_from_authenticated_domain` refuses anywhere but Primary;
  `two_admissible_observations_cannot_both_terminalize` in
  `formal/dclutch-semantics` is the machine-checked statement). Once complete
  has been observed and consumed, a later not-complete observation refuses
  without being inspected.

## What this is NOT

1. **Not devnet evidence and not mainnet evidence.** Two loopback validators.
2. **Not provider evidence.** The attestations are real Ed25519 signatures
   over bytes really read from the twin, CLAIMING the cluster the adapter
   release pins. That claim is a rehearsal fabrication, made loud: the daemon
   refuses the configuration unless every observed endpoint is loopback, and
   every artifact and publication-log line carries the rehearsal-twin label
   with both identities. The honest sentence about the strongest row is "the
   chain accepted a labelled rehearsal attestation asserting mainnet state".
3. **Not covered by the census where the daemon submits.** The daemon's
   appends and seal go straight from the daemon process to the validator; they
   are asserted by the campaign (the record must be Sealed) but do not appear
   in the evidence document the census binds.

## Registration

`bindings.json` binds this campaign's own labels (tier 1's are merged in front
at fold time, exactly as the journey does); `witnesses.json` runs under the
shared evaluator with the transcript as context. The runner is deliberately
its own script per `TIERS.md` — run.sh owns tier 1 and the census.
