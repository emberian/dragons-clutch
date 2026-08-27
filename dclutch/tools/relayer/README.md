# `dclutch-relayer`

The `RelayedMainnetStateV1` observation daemon (`docs/design/MAINNET_STATE_RELAY.md`
§4.11).

It reads account bytes off one Solana cluster and signs **observations** of
them. It never signs an interpretation. No field is selected, scaled, compared,
thresholded or named here: every layout fact — discriminators, admitted length
sets, field offsets, sentinels, fixed-point scale, rounding boundary — lives in
the `decoding_rules_id` record and is applied by the on-devnet adapter (§4.1).

Every byte it signs is encoded by `dclutch-relay-contract`. This crate
reimplements no offset, magic, domain separator or preimage.

## Why this is its own cargo workspace

`tools/relayer/Cargo.toml` carries an empty `[workspace]` table, exactly as
`crates/dclutch-svm-harness` does, and this crate is **not** in the root
`members` list.

The protocol workspace is `no_std`, `no_alloc`, fixed-layout kernel and contract
crates. This service needs `std`, tokio, an HTTP stack with TLS, and Solana
message construction — a heavy tree with a very different lock shape. The root
lock is also being actively realigned by other lanes; adding this dependency
tree there would collide with that work for no benefit. Promoting this crate
into the protocol workspace is a separate integration decision, not a side
effect of writing it.

It depends on the wire crate by path (`../../crates/dclutch-relay-contract`), so
there is exactly one copy of the ABI.

## What it will not do

These are the point of the design, not precautions bolted on:

- **It never looks for a wallet.** There is no default keypair path, no scan of
  `~/.config/solana`, no `id.json` fallback. The only paths it opens are the
  ones named in the config file, and it refuses even an explicitly named path
  that is inside `~/.config`, inside `~/.ssh`, or that traverses a
  `.config/solana` directory. `keygen` generates a fresh key; it is the only way
  this service obtains one.
- **It refuses a non-loopback submit endpoint** unless
  `allow_public_submission = true`, which defaults to false. That flag is an
  assertion that a current authorization names devnet or mainnet submission.
  Nothing in this service can grant that.
- **It refuses to run on the wrong cluster.** `expected_genesis_hash` is checked
  against `getGenesisHash` at startup on every configured endpoint. A mismatch
  is fatal, because nothing else distinguishes a mainnet account from a
  byte-identical twin elsewhere (§4.6).
- **Its dependency closure contains no venue IDL, SDK, or layout crate.** See
  `DEPENDENCY_CLOSURE.md`; §6.3 item 8 makes this release evidence.
- **It holds no market policy.** No thresholds, no windows, no staleness bounds,
  no schedule beyond "which account sets to observe, how often".

## Commands

```sh
# Generate a fresh test keypair at a path you name.
dclutch-relayer keygen --out ./keys/attestation.json

# Validate a config and print every derived identity, including the
# account_set_id you pin at founding.
dclutch-relayer show-config --config relayer.toml

# Observe, sign, write artifacts and the publication log. Submits nothing.
dclutch-relayer run --config relayer.toml --dry-run --cycles 1

# Additionally submit. Subject to the loopback gate above.
dclutch-relayer run --config relayer.toml --submit --cycles 1

# Read-only: sample two clusters' Clock sysvars and report max |a - b|.
dclutch-relayer measure-skew --endpoint-a URL --endpoint-b URL \
    --samples 12 --interval-seconds 30 --out-dir ./skew
```

`--cycles` defaults to **1**, and `--cycles 0` means "until interrupted". The
default is one on purpose: `AGENTS.md` requires public RPC reads to be explicit
and bounded, and a daemon whose default is an unbounded poll loop is neither.

`run` requires exactly one of `--dry-run` or `--submit`. There is no default
mode, because the difference between them is whether the process touches a
cluster.

## Configuration

See `relayer.example.toml`. Unknown fields are **refused**, not defaulted: a
typo'd `allow_public_submission` or `inline_len` that silently fell back to a
default is exactly the quiet wrong this family exists to prevent.

| Field | Meaning |
| --- | --- |
| `output_dir` | root for artifacts, `publication_log.jsonl`, `rpc_reads.jsonl` |
| `poll_interval_seconds` | seconds between cycles; must be ≥ 1 |
| `body_page_bytes` | one paged body read; must be ≥ 448 so the first page contains the whole pinned inline prefix |
| `observed_cluster.rpc_endpoints` | first is primary, the rest are cross-checks |
| `observed_cluster.expected_genesis_hash` | base58 or hex; verified at startup |
| `observed_cluster.request_timeout_seconds` | per-request HTTP timeout |
| `keys.attestation_keypair_path` | the release identity |
| `keys.fee_payer_keypair_path` | optional, hot and replaceable, must be a different file |
| `submit.endpoint` | optional; loopback unless explicitly authorized |
| `submit.allow_public_submission` | default **false** |
| `submit.relay_program_id`, `.market`, `.generation` | addressing for the append/seal routes |
| `submit.relayer_key_set`, `.relayer_key_set_staging_vacancy` | the raw immutable key-set record and its finalized staging vacancy |
| `submit.compute_unit_limit`, `.compute_unit_price_micro_lamports` | optional ComputeBudget preamble |
| `submit.address_lookup_table` | optional `{ key, addresses }` the v0 message compiles against |
| `account_sets[].name` | `[A-Za-z0-9_-]`; indexes artifact directories |
| `account_sets[].relay_family_id` | `ProviderReleaseV1.provider_family_id` |
| `account_sets[].decoding_rules_id` | `ProviderReleaseV1.decoding_rules_id` |
| `account_sets[].positions[]` | ordered `{ key, expected_owner, inline_len, admitted_data_lens }` |

`account_set_id` **cannot be configured.** It is derived from the ordered
positions via
`dclutch_relay_contract::release::encode_account_set_id_preimage_v1` and
SHA-256, and printed by `show-config`. A config that could state it would be a
second authority for which accounts may be attested.

`admitted_data_lens` is optional per position; empty means "any width that can
carry the pinned inline prefix". It is a *decoding-rules* fact the config echoes
so the daemon can stop early; the on-devnet adapter checks it again from the
pinned record and that check is the authority.

## The observation cycle

1. **One** `getMultipleAccounts` at `commitment: finalized` covering the entire
   set. Per-account `getAccountInfo` is forbidden for an observation: the batch
   call returns a single `context.slot` for the whole response, and a
   mixed-slot account set is the observation bug this family most needs to not
   have. One `dataSlice` covers the whole call, so it asks for the widest pinned
   `inline_len` in the set and truncates each position to its own width.
2. A Loader V3 `ProgramData` position (owner is the upgradeable loader,
   `inline_len` is 45) has its tail digest **cached** under
   `(programdata_pubkey, deployment_slot)`. `deployment_slot` sits at offset 4
   of the prefix the batch already fetched, so a redeploy is always detected
   from the prefix and a stale cached digest is unreachable. Any other position
   that is not fully inline is paged every cycle, because it has no cache key
   that a change is guaranteed to move.
3. Paging reads are `getAccountInfo` and are **not** the observation: they carry
   no slot into any signed message. What makes them safe is that the first page
   must reproduce the pinned inline prefix byte for byte as the batch read it,
   and every page must report the same `space`.
4. Bodies are built through `AccountObservationV1::new`, checked against the
   pinned position, folded into `set_digest`, wrapped in an
   `AttestationMessageV1`, signed, and verified against their own bytes before
   anything is written.
5. One `ObservationSetSealV1` per set, over the completed fold.

`space` is required in every RPC response. Under a `dataSlice` it is the only
place the account's complete width appears, and a guessed `data_len` would be an
interpretation. An endpoint that omits it is refused.

## Failure behaviour

On RPC disagreement, a missing account, a `data_len` outside the admitted set,
or a `deployment_slot` change, the daemon **stops attesting that set** and emits
a diagnostic. It never attests a partial or repaired observation, and a stopped
set is not recoverable inside the process. §4.11: the market's own funded
failure path (§4.8) is the correct handling of a stopped relayer, and it is
better than any repair the daemon could invent — a repaired observation is a
claim about mainnet that mainnet does not support, and nobody downstream can
tell it apart from a true one.

A **transport failure is deliberately not on that list.** A refused connection
is not a disagreement about what the cluster holds; it is the absence of an
answer, and the correct response is to try again next cycle. Only a cluster that
answered, and answered something inadmissible, stops a set.

Cross-endpoint agreement is byte-exact only when the two reads landed on the
same slot (the secondary is asked with `minContextSlot` set to the primary's
slot). When the slots differ, only existence, owner, `executable` and width are
compared — a mutable pool's bytes differing across slots is not a disagreement,
and treating it as one would stop every healthy set.

## Dry-run artifacts

One directory per set per cycle:

```text
<output_dir>/artifacts/<set-name>/slot-<observed_slot>/
    manifest.json                     everything below, tied together
    account_set_id.hex                the derived pin, one line
    observed_slot.txt                 the finalized slot, one line
    rpc_get_multiple_accounts.json    the verbatim primary RPC response
    rpc_cross_check.<host>.json       verbatim secondary responses, if any
    attestation.<index>.bin           exact signed message bytes
    attestation.<index>.sig           64 raw signature bytes
    seal.bin                          exact 156-byte seal message
    seal.sig                          64 raw signature bytes
```

`manifest.json` carries `artifact_schema: "dclutch.relayer.dry-run.v1"` and
spells every identifier in both hex and base58, so a `ProgramTest` campaign can
consume it without already knowing the format. Endpoint **hosts** are recorded,
never URLs: a provider URL commonly carries an API key.

Two different observations of one set at one slot is equivocation, and the
artifact tree is where it would first be visible, so a rewrite with a different
`set_digest` is refused and an identical one is a no-op.

## Publication log

`<output_dir>/publication_log.jsonl`, append-only, one line per signed message,
carrying the exact message bytes in hex, the message length, the signer, the
signature and the observed slot.

§4.11 requires that for every message it signs the daemon publish the exact
message bytes and the mainnet slot **to a public location**, and says plainly
that this is the entire mitigation for "the relayer can lie": an attestation
nobody can check against mainnet is a trust assumption; one that is published is
a falsifiable claim.

**Pushing this log to a public location is not implemented.** It is a separately
authorized act. Until it is done, this daemon does not satisfy §4.11's
publication requirement, and saying otherwise would be the claim the requirement
exists to prevent.

## Submission

Code-complete, execution-gated, and **not exercised against any cluster**.

Instruction order is `[compute budget…] · Ed25519 precompile · relay`, v0
message, over an optional Market ALT. The precompile is immediately before the
relay instruction by construction. The signed message sits at a fixed offset in
the relay instruction's data (`APPEND_OBSERVATION_PREFIX_BYTES = 40`,
`SEAL_RECORD_PREFIX_BYTES = 32`), and the descriptor's `message_data_offset` is
that constant — which is what lets the on-chain adapter compare it rather than
trust it. Account frames come from `dclutch_relay_contract::frame`, not from a
table copied into this crate, so the daemon cannot drift from the frame the
program validates.

On blockhash expiry the daemon **re-signs the built transaction and never
re-observes.** The attestation is bound to `observed_slot`; taking a fresh read
because a blockhash aged out would silently change the fact being attested.

Only the **append** and **seal** routes are built. Record creation and
retirement are not, so the observation record must already exist for the
`(market, generation, account_set_id, observed_slot)` being submitted. The
daemon says so at startup when `--submit` is passed.

## Tests

Offline, no network. `cargo test` runs 78 tests, including one integration test
that stands up a loopback mock RPC and drives a complete cycle: batch read,
pinned-prefix truncation, three-page tail digest, fold, encode/sign/verify round
trip through the wire crate's own decoder, seal, artifact directory and
publication log.

Gates: `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`,
`cargo fmt --check`. Non-test code is held to the protocol workspace's lint bar
(`[lints]` in `Cargo.toml`): no `unwrap`, no `panic`, no indexing, no truncating
casts.
