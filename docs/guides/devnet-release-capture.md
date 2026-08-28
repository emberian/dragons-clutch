# Capture the live devnet release inputs

These two commands read Solana devnet and write the account evidence needed by
the permanent-program update workflow. They cannot sign or submit a
transaction, and they accept only the canonical public devnet endpoint.

## Capture Registry and Rent

Run:

```text
dclutch-local-successor-bootstrap devnet-carry-forward-capture-v1 \
  --rpc-url https://api.devnet.solana.com \
  --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \
  --expected-registry-program REGISTRY_PROGRAM \
  --expected-rent-program RENT_PROGRAM \
  --expected-core-program CORE_PROGRAM \
  --expected-upgrade-authority RETAINED_AUTHORITY \
  --minimum-context-slot FINALIZED_FLOOR \
  --output /absolute/new/carry-forward-snapshot.json
```

The command first reads the Registry and Rent Loader pairs and the Core-owned
infrastructure profile. It decodes that live profile to derive both artifact
record addresses and both staging addresses. It then reads the final nine
accounts together at or after the discovery slot. The saved document contains
only that final context.

The command refuses unless both Program accounts link to their derived
ProgramData accounts, both ProgramData bodies carry the authority you named,
both artifact records authenticate those deployments, the profile selects the
same programs and artifacts, and both staging accounts are RPC `null`.

## Capture the five updated ProgramData bodies

Run:

```text
dclutch-local-successor-bootstrap devnet-prepare-programdata-capture-v1 \
  --rpc-url https://api.devnet.solana.com \
  --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \
  --expected-custody-program CUSTODY_PROGRAM \
  --expected-resolution-program RESOLUTION_PROGRAM \
  --expected-claims-program CLAIMS_PROGRAM \
  --expected-trading-program TRADING_PROGRAM \
  --expected-core-program CORE_PROGRAM \
  --expected-upgrade-authority RETAINED_AUTHORITY \
  --minimum-context-slot FINALIZED_FLOOR \
  --output-dir /absolute/new/programdata-capture
```

The command reads all five Program/ProgramData pairs in one finalized request.
It verifies their Loader owners, privileges, derived links, deployment slots,
complete bodies, and retained authority before it writes anything.

The new directory contains these raw inputs for checked `prepare`:

```text
00-custody-programdata.bin
01-resolution-programdata.bin
02-claims-programdata.bin
03-trading-programdata.bin
04-core-programdata.bin
manifest.json
```

The manifest records the one context slot, exact ordered addresses, body and
live-executable digests, byte lengths, deployment slots, and a digest over the
whole bundle. It is written last and fsynced as the completion marker. If the
directory or JSON target already exists, the command refuses without changing
it.

RPC responses are parsed from their original bytes. Duplicate object keys,
trailing JSON values, wrong request identifiers, malformed account encodings,
and a response below your context floor all refuse.
