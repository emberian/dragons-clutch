# Devnet activity reconciler

This tool reads finalized Solana RPC evidence independently of the SDK, web
client, CLI projections, or an indexer. It checks one complete activity chain
and writes a deterministic, unsigned dossier. Public devnet and an owned local
validator are separate input and output types; local evidence is never labeled
public, live, or devnet. It never reads a key, builds a transaction, signs,
submits, or mutates an account.

The current lifecycle tools persist operation-scoped evidence. A small adapter
projects the exact facts owned by those journals into the cluster-specific
reconcile manifest. Every projected event carries the SHA-256 digest of its
source journal. The reconciler does not trust a static client as a source of
chain truth. The owned-loopback producer remains gated on the frozen Direct
completion schema and a successful full private-validator run. Synthetic
receipt tests prove the parser, not a completed protocol lifecycle.

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
- for owned-loopback Hot evidence, exact ordered seller and buyer Position
  pre/post states, distinct accounts and owners, and one conserved outcome
  transfer equal to the fill; the public-devnet manifest does not admit this
  private-only field;
- exact hostile decoding and market binding of `ResolutionCertificateV2`;
- every payout's claim burns and equal Hoard-principal debit / recipient credit;
- retirement closure observations and exact positive refund deltas;
- current finalized raw account bytes, owner, lamports, Token-2022 base fields,
  or vacancy, at a slot no earlier than the activity.

Hoard principal, protocol fees, and transaction fees are reported as three
different quantities. The dossier has `signatureScheme: "none"`; its public
transaction signatures are evidence identifiers, not a signature over the
dossier. `evidence.rpc.mode` distinguishes a reproducible captured-RPC replay
from a live finalized RPC observation. The former binds the exact capture-file
SHA-256 and does not claim that replay was a new live observation. The latter
publishes only the endpoint SHA-256, not a potentially credential-bearing URL.
`dossierSha256` hashes canonical dossier JSON before that field is added.

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

An owned local validator uses a different command and four different schemas:

```sh
python3 tools/devnet-reconcile/reconcile.py owned-loopback-captured \
  --manifest owned-loopback-activity-manifest.json \
  --rpc-capture evidence/finalized-rpc.json \
  --session-receipt owned-loopback-session-receipt.json \
  --expected-session-receipt-sha256 <sha256-from-private-summary> \
  --evidence-root evidence \
  --out owned-loopback-activity-dossier.json
```

The command accepts no RPC URL. Its manifest must use
`dclutch-owned-loopback-activity-reconcile-manifest-v1`; its capture must use
`dclutch-owned-loopback-captured-finalized-rpc-v1`; its authenticated receipt
must use `dclutch-owned-loopback-reconcile-session-receipt-v1`; and its output
uses `dclutch-owned-loopback-activity-dossier-v1`. Public devnet and mainnet
genesis hashes refuse on all three local inputs. The receipt SHA-256 is an
independent required argument, so changing an ELF, ProgramData genesis digest,
journal, capture, or receipt after PRIVATE publishes its digest refuses.
The owned-loopback `--out` path must be absent and absolute beneath an existing
canonical ordinary parent directory. The dossier is published without
clobbering at mode `0600`; an existing file, output symlink, or symlinked parent
refuses.

The receipt carries one clean source commit, checked-release digest, the exact
ordered seven dClutch plus Pyth Receiver/Router program closure, each Program
and ProgramData identity, deployment slot, ELF digest, and genesis ProgramData
digest. It separately reopens the immutable local Pyth provider closure, its
checked successor plan and local-validator profile, and the same singular
finalized capture. It also binds the exact manifest bytes, canonical ordered
source-journal set, one finalized eight-stage PRIVATE activity session, and one
separately typed eight-stage chaos session. Every activity-stage source is
reopened by path, SHA-256, schema, completion pointer, and finalized value;
`stageSetSha256` is recomputed. Missing programs or journals, substituted bytes,
provisional evidence, nested session completion substitutions, and partial
lifecycle or chaos stage sets refuse before reconciliation.

The receipt must include exactly one
`dclutch-owned-loopback-terminal-sequence-completion-v1` journal. The reconciler
does not stop at its `/status`: it reopens the terminal session and every
ordered internal mutation journal, checks their SHA-256 values, and compares
the completion projection with the persisted mutation, fee payer, signature,
slot, fee, compute units, and canonical protocol-lamport deltas. It requires
the exact lookup create/strict-prefix extends/freeze sequence when the caller
created a table, at most one receipt prepay, and the six retirement mutations
in their sole order. Aggregate slot, fee, and compute arithmetic is recomputed;
self-consistent edits to the completion that disagree with a source journal
refuse.

Each journal row names an exact RFC6901 `completionPointer` and requires the
pointed value to be `"finalized"`. This admits Direct's nested Hot journal
without pretending its aggregate evidence owns a top-level phase. The activity
completion stage projection is exactly founding, participant, ALT, seal,
Direct, resolution, payout, and retirement; the separately typed chaos session
may retain its own Hot/retire vocabulary.

All eighteen Program/ProgramData accounts must be present in the finalized
capture. The consumer hostile-decodes each Loader-v3 Program link and
ProgramData header, then recomputes the full ProgramData and ELF-tail digests.
It also decodes the ProgramData upgrade-authority option: the seven dClutch
programs must share one non-null retained disposable authority, while Pyth
Receiver and Router must be immutable slot-zero genesis programs. A slot-zero
dClutch deployment or a mutable local provider refuses. Receipt strings alone
are not Loader evidence.

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

The devnet capture schema is `dclutch-captured-finalized-rpc-v1`. Its `transactions`
map contains unmodified JSON-encoded `getTransaction` results keyed by first
signature. Its `accounts` map contains
`{"contextSlot":"...","value":<getAccountInfo value>}` keyed by address.
Strict JSON duplicate keys, unknown manifest fields, duplicate identities,
forks, missing evidence, mixed mints, and substituted raw state all refuse.
Source paths are confined beneath `--journal-root`; absolute paths, traversal,
symlink escapes, non-JSON journals, and digest substitutions refuse before any
RPC read.

The owned-loopback capture additionally carries `commitment: "finalized"` and
one `finalizedSlot`. Every account row must have that exact `contextSlot`; a
mixture of individually finalized account reads is not a singular capture.
Every captured transaction must be at or below that boundary. Its dossier says
`owned-loopback-local-evidence-not-public-devnet-or-live-observation` and uses
`owned-loopback-captured-finalized-rpc-replay` provenance.

Run the local hostile corpus with:

```sh
python3 -m unittest discover -s tools/devnet-reconcile/tests -v
```
