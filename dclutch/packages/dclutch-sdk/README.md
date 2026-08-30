# @dclutch/sdk

The read-first client surface of the dClutch protocol: generated ABI modules,
hostile decoders, exact arithmetic previews, and transaction surfaces only
when their caller owns the complete journal and finalization boundary. The CLI
imports this package. The web app still carries tracked mirrors while the
consumer flip is unfinished; `scripts/sync-from-web.mjs` reports that drift and
must be read before claiming the two surfaces have converged.

## Contract

- **Connection-agnostic.** Nothing here constructs an RPC connection, reads a
  wallet, or touches a browser global. `SolanaRpcClient` takes an endpoint
  (and optionally a `fetch`); every chain-reading function takes the client
  as an argument. The package typechecks against the plain node lib set —
  `dom` is deliberately absent from `tsconfig.json`, so a browser dependency
  cannot land here silently.
- **Caller-complete mutation only.** Direct exposes route authentication,
  intent bytes, and exact arithmetic preview, not its internal unsigned packet
  compiler. The wallet subpath exposes unsigned-packet inspection, not generic
  signing or submission. A mutation surface opens only with its durable phase
  journal, exact returned acknowledgement, and finalized poststate proof.
- **Generated truth.** `lib/generated/` is emitted from the protocol's own
  authorities — Lean schemas via `scripts/lean-emit.mjs`, Rust contract
  sources via the `scripts/generate-*.mjs` scrapers — and every module has a
  byte-comparison `abi:<surface>:verify` script. All of them run inside
  `npm test`, so a green suite implies the ABI mirrors are current. The
  scripts read `../../crates`, `../../programs`, `../../formal` and
  `../../docs/reference`: **this package regenerates only inside the
  repository** (published tarballs would carry the committed, verified
  modules).
- **The ratchet.** `npm run abi:coverage` inventories every protocol fact
  still stated by hand (record magics, seed domains, literal byte offsets);
  `lib/abiCoverage.test.ts` fails when the inventory grows.

## Checked live-devnet read

`LIVE_DEVNET_OPERATOR_PRESET_V1` supplies the canonical public endpoint and
the six operator-role coordinates already owned by `lib/deployments.ts`.
Pass it to `acquireOperatorSurfaceV1` with a `SolanaRpcClient` to reacquire
devnet genesis, the exact Loader Program links, six canonical ProgramData
PDAs, their 45-byte slot/authority headers, and the complete Registry
activation cache at finalized commitment. The read deliberately uses data
slices for ProgramData, so it never downloads the multi-megabyte ELF bodies.

The returned `routeSpecificReleaseAdmission` remains `unproven`. A matching
program generation does not invent a Realm or Market and does not authenticate
the releases selected by a particular route.

## Release identity and ABI selection

`lib/releaseIdentity.ts`. A dClutch program is upgraded in place — the
permanent-ID ladder keeps all seven addresses across every cohort, deliberately
— so a program address tells a client nothing about which code it is talking
to. Ask the chain instead:

```ts
const session = await openReleaseBoundSessionV1(client, {
  registryProgram: deployment.programs.registry,
  activationCache: deployment.activationCache,
});
session.abi.coreFoundAccountCount; // the frame this release actually speaks
```

Two bounded RPC rounds, once per session, before the first frame is built. The
first reads the Registry's `DCLTACT1` activation cache through the contract's
own hostile decode; the second confirms that cache is the CURRENT one by
comparing its five pinned deployment slots against the live ProgramData
accounts.

**The manifest's `activationCache` is a bootstrap HINT, not the answer.** Each
cohort activates a new release set, which mints a new cache at a new PDA, so a
baked address ages out the moment a cohort lands — and superseded caches are
never deleted, keeping their Registry owner, their `DCLTACT1` magic and their
exact width forever, so existence, owner and magic cannot tell you a cache is
current. When the hint has aged out, the session FOLLOWS the chain instead of
refusing: `discoverCurrentActivationCacheV1` reads every 1288-byte account the
Registry owns and takes the one whose five pinned slots equal the five live
ProgramData slots. Still two rounds regardless of cohort count, because the
program addresses are permanent, so every cohort names the same five
ProgramData accounts. `session.source` says which path was taken and names both
caches when it followed. Pass `followCurrent: false` to refuse instead.

Discovery finding NO coherent cache is a real alarm, not a fallback: it means
the deployment was upgraded without re-activating a release set, and no client
can bind a frame to a release the chain is not running. That refusal, and the
unknown-identity refusal, are the point — they replace a frame mismatch
surfacing as an opaque `0x4001` deep inside a composed on-chain instruction,
diagnosable only by archaeology across client and program git history.

Tables are keyed on each role's `semantic_release_id`, not on the release-set
id. A release set id is the hash of the whole activated set, so it moves on any
rebuild — keying on it would make clients refuse on every cohort bump, which is
breakage on a schedule rather than upgrade-proofing. The semantic release id is
derived from the role's source, which is exactly what a generated ABI table
describes; observed on devnet, Trading and Resolution held one semantic release
id across four consecutive cohorts while their ELF digests and slots moved every
time. This is the protocol's own idiom: `authenticate_role_semantic_release` in
`dclutch-resolution-core-v3-operator` performs the same refusal on chain.

Scope, stated narrowly: this guarantees a client REFUSES TO ACT against a
release it was not built and pinned for. It does not verify that a pinned
table's frames are correct for that release — that is the `abi:*:verify`
generator gates' job. It also does not close `routeSpecificReleaseAdmission`,
which is about a particular route's selected releases, not the activated set.

### Shipping a new cohort

One step. A new cohort activates a new release set, which mints a new activation
cache at a new PDA and may move any role's semantics.

1. Regenerate the ABI modules: `npm run abi:<surface>` for whatever moved (or
   all of them), and let `abi:*:verify` prove they match the Rust authorities.
2. Append one `AbiReleaseTableV1` to `KNOWN_ABI_RELEASES_V1`, with the five
   semantic release ids **read off the live activation cache**, not transcribed
   from a plan. `DCLUTCH_LIVE_DEVNET=1 npx vitest run lib/releaseIdentity.live.test.ts`
   prints them in its refusal when the identity is unknown.

Do not edit an existing entry — it describes a release that really ran — and do
not add tables for releases this repository never observed. A fabricated table
is worse than a refusal: it selects silently, and it is wrong. There is no era
reasoning anywhere in this: no client-vs-program git archaeology, no coherent
build windows, no composite clients.

Finding the current cache, if the deployment manifest's `activationCache` has
aged past it: read the Registry program's accounts of width 1288 — one exists
per cohort, and superseded ones are never deleted — and take the one whose five
pinned deployment slots equal the five live ProgramData deployment slots. A
superseded cache keeps its Registry owner, its `DCLTACT1` magic and its exact
width forever, so existence, owner and magic cannot tell you it is current.

## Versioning

`0.1.0`, and honestly so: the API is the extraction of a moving application
surface, not a stability promise. The package is marked `private` because
publishing to a registry is an explicit dispatch decision (see
`AGENTS.md`: never publish without current authorization naming the act) —
flipping `private` and choosing a scope owner is that decision, not a build
step.

## Layout

- `index.ts` — curated root exports. Public subpaths resolve through
  `package.json`; `directInlineV3` and `walletHandoff` deliberately resolve to
  read-only facades rather than their internal conformance modules.
- `lib/` — the modules and their tests, colocated.
- `lib/generated/` — emitted ABI mirrors; regenerate with `npm run
  abi:<surface>`, never edit.
- `lib/refusals.ts` + `lib/generated/refusalRegistryV1.ts` — refusal codes
  rendered by name via the band registry (decision 0007).
- `fixtures/` — the committed evidence fixtures the tests and the
  local-successor conformance check read.
- `scripts/` — the generators, their verify gates, and the coverage ratchet.

What stayed in the web app: `lib/walletStandard.ts` (browser wallet
discovery is a browser concern), the React components and routes, and the
repo-wide SBOM gate.
