# Capture the live devnet release inputs

These three commands read Solana devnet and write the account evidence needed by
the permanent-program update workflow. They cannot sign or submit a
transaction, and they accept only the canonical public devnet endpoint.

## Capture the complete pre-write substrate

Run this immediately before you start the first Upgrade role:

```text
dclutch-local-successor-bootstrap devnet-permanent-substrate-capture-v1 \
  --rpc-url https://api.devnet.solana.com \
  --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \
  --expected-upgrade-authority RETAINED_AUTHORITY \
  --fee-payer FEE_PAYER \
  --minimum-context-slot FINALIZED_FLOOR \
  --output /absolute/new/permanent-substrate-snapshot.json
```

You do not supply program addresses. The command owns the fixed decision-0012
table of seven permanent Program/ProgramData pairs and reads all fourteen
Loader accounts plus the explicit fee payer in one finalized
`getMultipleAccounts` context. It verifies every Loader owner, privilege,
derived ProgramData coordinate, Program link, nonzero deployment slot,
retained authority, rent-exempt balance, full ProgramData digest, and live ELF
digest. It reports the exact Program and ProgramData lamport totals and the fee
payer balance from that same context.

The output carries a domain-separated canonical `snapshot_sha256`. It is a
compact pre-write gate, not another owner of the large account bodies: use the
two captures below for the exact body bytes consumed by CarryForward and
checked `prepare`. A caller-supplied program flag, keypair flag, non-devnet
endpoint, missing payer, or changed fixed Loader pair refuses.

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
