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
