# Local-real Pyth multi-boundary Source V2 campaign

Status: **IMPLEMENTED / HOST-TESTED / LOCAL VALIDATOR RUN NOT YET RETAINED**.

This lane extends the existing non-production local-real Pyth laboratory
without changing the historical one-boundary transcript schemas. It remains
entirely local and uses synthetic observations signed by deterministic
laboratory guardians. The local fixture reconstructs complete loader bodies
from SHA-pinned Pyth router/receiver ELFs plus decoded ProgramData keys,
deployment slots, and upgrade authority. Raw `solana account` JSON responses
were not retained. The real ABIs and cryptographic verification execute only
when the clean-HEAD runner is invoked. This is not devnet price,
provider-availability, current deployment, production, mainnet, or demand
evidence.

## Scope

`joined-multiboundary-v1` freezes a two-bucket window ending at the most recent
named closing boundary. It generates two distinct 13-of-19 VAAs for the
registered feed at consecutive 60-second publish times. Each update has
`prev_publish_time = publish_time - 1`, so each body witnesses exactly its own
closing boundary under `CROSSING_V1`; neither is a fabricated archive record.

The campaign then:

1. verifies both signed VAAs through the captured router binary;
2. attempts the second boundary before the first and requires the adjacent
   receiver `PostUpdate` plus Clutch `AppendSourceArchiveV2` transaction to
   refuse with `SourceAdmissionFailed` and roll back the receiver account,
   archive, and treasury state;
3. posts and appends both boundaries in canonical order through the captured
   receiver binary and authenticated Source V2 ABI join;
4. authenticates the open archive envelope and complete page through the
   canonical Source V2 codec (exact owner, executable bit, 2,560-byte length,
   header identities, record lineage, zero unused tail, and commitment);
5. seals and reloads the page through
   `verify_recorded_sealed_archive_v2_view`, then reads both records through
   its bounded canonical accessor and checks bucket, conservative integer
   interval, sequence, receiver write slot, and publish time;
6. runs the signed two-owner general trade,
   resolves the exact categorical payout from the sealed source window, redeems
   both owners, and withdraws all 128 collateral atoms.

The source-authentication checks are unchanged. Wrong Config and wrong feed
remain fail-closed rollback probes. The new out-of-order probe adds a lineage
failure; it does not relax freshness, identity, adjacency, ownership,
executable-bit, loader, ABI, or cryptographic checks.

## Reproduction

The host-only gate is offline and works in a dirty tree:

```sh
CARGO_NET_OFFLINE=true cargo +1.93.1 test --locked --offline \
  --manifest-path programs/clutch-sbf/local-real-pyth/Cargo.toml
bash -n programs/clutch-sbf/scripts/run_local_real_pyth.sh \
  programs/clutch-sbf/scripts/run_local_multiboundary_pyth_lifecycle.sh
```

The material local-validator gate intentionally requires every tracked build
input to be clean at `HEAD`. After these files are coherently committed and
the concurrent `crates/`, `programs/clutch-sbf/program/`, and
`programs/solana-layout/` work named by the base runner has also landed or
otherwise returned clean, run:

```sh
transcript_dir="$(mktemp -d "${TMPDIR:-/tmp}/clutch-pyth-multiboundary.XXXXXX")"
CLUTCH_LOCAL_REAL_PYTH_TRANSCRIPT_DIR="$transcript_dir" \
  programs/clutch-sbf/scripts/run_local_multiboundary_pyth_lifecycle.sh
```

The runner uses only its provenance-checked loopback validator, private
temporary signer directory, repository-local vendored dependencies, and
captured provider fixtures. It refuses a non-loopback RPC URL, an untracked or
dirty campaign input, lockfile drift, mismatched provider/runtime hashes, and
any transcript destination containing one of its five target filenames.

## Remaining gates

- **P0:** coherently commit the campaign files and let the concurrent dirty
  build-input lanes under `crates/`, `programs/clutch-sbf/program/`, and
  `programs/solana-layout/` land or return clean so the clean-HEAD provenance
  gate can run; then retain and audit one successful 56-step transcript.
- **P0 if the run fails:** diagnose without bypassing the clean-source,
  loopback-listener, source-authentication, or atomic-rollback checks.
- **P1:** add a strict client reader after a retained v1 transcript exists.
  Historical joined-v4 evidence must not be reinterpreted as multi-boundary
  evidence.
- **P1:** both boundaries intentionally carry the same deterministic price.
  This proves two distinct signed VAA/update bodies, consecutive lineage, and
  multi-record consumption, but not a price-changing cross-boundary fold. Add
  a distinct-price compatible interval campaign later; it is not evidence
  claimed by this wave.
- **Production STOP:** no production SourceSpec is registered, and these
  synthetic local guardian payloads provide no production or devnet source
  evidence.

## Retained result schema

The uncommitted/unrun producer keeps schema
`dragons-clutch/operator/local-real-pyth-multiboundary-joined-lifecycle/v1`.
Its multi-boundary additions are fixed before the first retained run:

- `archive_records` is ordered by record index. `bucket`, `lower`, `upper`,
  `sequence`, `write_slot`, and `publish_time` are exact decimal strings so a
  JavaScript reader cannot round integers.
- `source_archive` retains the account key, Clutch owner, executable bit,
  exact body length, complete-body SHA-256, recomputed page commitment, feed
  identity, window identity, and record count. These fields are emitted only
  after the canonical sealed verifier authenticates the exact bytes.
- `wrong_config_rollback`, `wrong_feed_rollback`, and
  `out_of_order_boundary_rollback` share one exact object shape in this schema.
  Each retains a closed attempt kind and exact attempted/registered identity,
  the ephemeral receiver-update address and explicit post-refusal absence, the
  ordered-step label and signature, instruction index `2`, custom code `122`,
  and ordered watched accounts (archive then receiver treasury). Equal
  before/after SHA-256 values cover each account key, presence, lamports,
  owner, executable bit, data length, and complete data under domain
  `dragons-clutch/local-real-pyth/rollback-snapshot/v1`. The producer refuses
  malformed digests, aliased watched accounts, equal wrong/correct identities,
  an out-of-order index/publish-time pair inconsistent with the campaign's
  60-second boundary schedule, or unequal snapshots before it can serialize a
  success object. The out-of-order identity may name any later scheduled
  boundary; it does not freeze the schema to this first two-boundary fixture.
- `lifecycle.terminal.liabilities` is built only after both withdrawals reload
  the final SupplyLedger and all four authoritative Token-2022 outcome mints.
  It retains the ledger address and outcome count, every internal,
  external-ledger, and aggregate supply, and each indexed mint address and
  actual supply. All supply values are canonical decimal strings. The producer
  refuses to emit the object unless the ledger binds the resolved market, its
  active width matches the mint vector, addresses are nonzero and distinct,
  canonical padding is zero, and every retained liability value is exactly
  zero. The joined-v4 producer executes these terminal checks without adding
  the new multi-boundary-only object to its historical schema.

Historical `source-only-v1` and `joined-user-lifecycle-v1` schemas retain their
boolean `wrong_config_rollback` and `wrong_feed_rollback` fields. Only the
separately named multi-boundary v1 schema uses the three structured objects.

The runner may use a destination directory that already exists only when all
five target filenames are absent: `campaign.json`, `result.json`,
`probe-evidence.json`, `probe-before.txt`, and `probe-after.txt`. It refuses to
overwrite any one of them. Before any copy, the runner builds those five files
in an ephemeral public staging directory and audits their final bytes. Raw
listener probes and the validator log remain in the private work directory;
the retained probe summaries contain only the exact loopback endpoints,
per-endpoint and total observation counts, selected-validator hash, result,
and raw/log SHA-256 provenance. `campaign.json` reduces the selected validator
path to its basename while keeping the complete binary SHA-256 as its identity.
The gate rejects absolute home paths, the local username, non-loopback literal
IPs, PID/FD rows or JSON fields, and keypair/private-key/secret markers.
