# Private-validator Loader-v3 Upgrade rehearsal

This is the interruption/recovery controller for the final private-validator
Upgrade rehearsal. It is local evidence, not devnet admission, and it never
reads a key, creates a Solana instruction, chooses an account, or talks to a
non-loopback RPC endpoint. Those operations remain behind one injected driver
whose signed packet and signature are frozen before its only send attempt.

The controller admits exactly seven rows, in this order:

1. Registry — `carry_forward`
2. Rent Credit — `carry_forward`
3. Custody — `upgrade`
4. Resolution — `upgrade`
5. Claims — `upgrade`
6. Trading — `upgrade`
7. Core — `upgrade`

Every row binds its existing Program, ProgramData, deployment slot, retained
upgrade authority, and live payload SHA-256. Each Upgrade row additionally
binds an exact non-symlink payload file, persistent Buffer, Buffer authority,
byte count, and target payload SHA-256. Programs and ProgramData never change;
there is no recycle path. The two infrastructure rows are observed before the
first mutation and again after all five activations.

## Durable recovery

For every Buffer create, Buffer write, Loader-v3 Upgrade, and activation, the
driver first returns the exact signed packet, packet digest, signature, recent
blockhash, and last-valid block height. The controller fsyncs those facts as
`signed_not_submitted`, then fsyncs `dispatching`, then polls the signature.
Only an absent and unexpired signature permits one send of the identical
packet. `submitted` recovery is poll-only. An expired unfinalized packet is a
refusal, not an excuse to re-sign an ambiguous intent.

Recovery never trusts the journal alone. It reobserves the Buffer's exact
authority, capacity, uploaded byte count, and prefix SHA after every chunk. If
Upgrade reached the validator before the response or phase write, restart
accepts the new payload only when the already-fsynced exact Upgrade signature
is finalized. It then records the exact post-Upgrade ProgramData slot and
account digests, activates that checked release, and reobserves the immutable
activation record.

The deterministic crash names are:

```text
after_buffer_create:<role>
after_buffer_write:<role>:<zero-based-chunk>
before_upgrade_send:<role>
after_upgrade_send:<role>
after_postcapture:<role>
after_activation:<role>
```

A driver timeout, nonzero exit, or malformed response at `send_transaction`
is treated as response loss. The controller exits with the journal still in
`dispatching`; the next invocation polls that same signature before doing
anything else.

## Driver contract

The driver is one absolute, regular, executable file. Each invocation receives
one unique-key canonical JSON request on stdin:

```json
{
  "schema": "dclutch-private-loader-v3-rehearsal-driver-request-v1",
  "operation": "observe_role",
  "rpcUrl": "http://127.0.0.1:8899",
  "body": {}
}
```

It returns exactly:

```json
{
  "schema": "dclutch-private-loader-v3-rehearsal-driver-response-v1",
  "operation": "observe_role",
  "body": {}
}
```

Supported operations are `observe_role`, `observe_buffer`,
`prepare_transaction`, `get_block_height`, `poll_transaction`,
`send_transaction`, and `observe_activation`. The body schemas are enforced by
`rehearsal.py`. The accepted final driver should delegate instruction and
Loader-history validation to the existing successor Upgrade owner; it must not
grow a second account map. The test driver is deliberately in-process and has
no authority outside its temporary directory.

## Run

The spec and journal paths must be absolute. The journal parent must already
exist and must not be a symlink.

```sh
python3 tools/release/private_validator_upgrade/rehearsal.py \
  --spec /absolute/private-upgrade-spec.json \
  --journal /absolute/fresh-or-resumed/journal.json \
  --driver /absolute/accepted-private-upgrade-driver
```

At final source freeze, generate the spec from the checked release gate and a
fresh chain capture, inject the exact five final ELFs into its payload rows,
and run every named interruption against disposable owned validators. The
mechanism itself is source-independent and can be tested now:

```sh
python3 -m unittest tools/release/private_validator_upgrade/test_rehearsal.py
```

These tests exercise ordinary completion, all durable boundary classes,
post-send process death, response loss after chain mutation, packet expiry,
ProgramData substitution, hostile Buffer bytes, journal corruption, final
CarryForward drift, policy/order drift, payload drift, and non-loopback RPC
refusal.
