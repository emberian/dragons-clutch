# Devnet activity reconciler

This tool reads finalized Solana RPC evidence independently of the SDK, web
client, CLI projections, or an indexer. It checks one complete activity chain
and writes a deterministic, unsigned public dossier. It never reads a key,
builds a transaction, signs, submits, or mutates an account.

The current lifecycle tools persist operation-scoped evidence. They do not yet
publish one frozen aggregate activity schema. A small adapter must therefore
project the exact facts owned by those journals into
`dclutch-activity-reconcile-manifest-v1`. Every projected event carries the
SHA-256 digest of its source journal. The reconciler does not accept the
tentative private-validator session spelling and does not trust a static client
as a source of chain truth.

## What is checked

- every transaction in an ordered founding, participant, Direct, resolution,
  payout, and retirement chain, joined by a single predecessor chain (a phase
  may own multiple transactions and multiple transaction fees);
- sequential pre/post continuity for every wallet lamport balance, token atom
  balance, and Position touched more than once, joined to the current finalized
  token/Position state so individually valid transactions cannot be spliced;
- the exact devnet genesis hash and finalized transaction signature, slot,
  status, transaction fee, account vector, lamport deltas, and raw token atoms;
- all changed lamport and token accounts are declared—an unexplained balance
  change refuses;
- every token account bound to its declared mint, authority, token-program
  owner, and either the collateral or claim asset class; every Direct and
  payout role bound to the one Realm collateral mint (claim mints may
  correctly differ);
- every Direct fill's gross quote with no unnamed rounding, then an independent floor of
  `gross * 50 / 10_000` on each side;
- exact `LiabilityBasisPositionV2` identity, geometry, revision, and balances;
- exact hostile decoding and market binding of `ResolutionCertificateV2`;
- every payout's claim burns and equal Hoard-principal debit / recipient credit;
- retirement closure observations and exact positive refund deltas;
- current finalized raw account bytes, owner, lamports, Token-2022 base fields,
  or vacancy, at a slot no earlier than the activity.

Hoard principal, protocol fees, and transaction fees are reported as three
different quantities. The dossier has `signatureScheme: "none"`; its public
transaction signatures are evidence identifiers, not a signature over the
dossier.

## Commands

Captured evidence is the reproducible gate:

```sh
python3 tools/devnet-reconcile/reconcile.py captured \
  --manifest activity-manifest.json \
  --journal-root evidence \
  --rpc-capture finalized-rpc.json \
  --out public-activity-dossier.json
```

Bounded live polling performs only `getGenesisHash`, finalized
`getTransaction`, and finalized `getAccountInfo` calls. HTTPS, 1–30 polls,
0–60 second intervals, and 1–30 second request timeouts are hard limits:

```sh
python3 tools/devnet-reconcile/reconcile.py follow \
  --manifest activity-manifest.json \
  --journal-root evidence \
  --rpc-url https://api.devnet.solana.com \
  --max-polls 5 --interval-seconds 2 --timeout-seconds 10 \
  --out public-activity-dossier.json
```

Live follow intentionally refuses until every operation has a finalized source
journal and the adapter supplies the complete six-event chain. In particular,
the current Direct public manifest is not execution evidence.

## Adapter and capture shapes

All integer quantities in the adapter are canonical decimal strings. Account
references are unique logical names whose addresses may not alias. The manifest
contains:

- `cluster`: exact `devnet` kind and genesis hash;
- `accounts`: `{ref,address,kind,role}` entries, with exact `mint`, `authority`,
  `programOwner`, and `assetClass` (`collateral` or `claim`) on every token
  account;
- `sourceSetSha256`: SHA-256 of the canonical ordered
  `[{"event":...,"sha256":...}]` source list;
- `events`: one or more events in each of the six canonical phases, each with
  an operation name, canonical relative `sourcePath`, digest of that exact
  strict-JSON journal, finalized identity, exact lamport/token deltas, and its
  kind-specific facts;
- `finalAccounts`: exact current owner/lamports/data digest and Token-2022 fields,
  or `closed: true`.

The capture schema is `dclutch-captured-finalized-rpc-v1`. Its `transactions`
map contains unmodified JSON-encoded `getTransaction` results keyed by first
signature. Its `accounts` map contains
`{"contextSlot":"...","value":<getAccountInfo value>}` keyed by address.
Strict JSON duplicate keys, unknown manifest fields, duplicate identities,
forks, missing evidence, mixed mints, and substituted raw state all refuse.
Source paths are confined beneath `--journal-root`; absolute paths, traversal,
symlink escapes, non-JSON journals, and digest substitutions refuse before any
RPC read.

Run the local hostile corpus with:

```sh
python3 -m unittest discover -s tools/devnet-reconcile/tests -v
```
