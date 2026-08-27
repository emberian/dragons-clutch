# @dclutch/sdk

The client surface of the dClutch protocol: the generated ABI modules, the
proven transaction builders, and the hostile decoders that grew up inside
`apps/dclutch-web/lib/` as the de facto SDK. This package is their one home;
the web app imports them from here.

## Contract

- **Connection-agnostic.** Nothing here constructs an RPC connection, reads a
  wallet, or touches a browser global. `SolanaRpcClient` takes an endpoint
  (and optionally a `fetch`); every chain-reading function takes the client
  as an argument; every builder returns instructions or transactions for the
  caller to sign. The package typechecks against the plain node lib set —
  `dom` is deliberately absent from `tsconfig.json`, so a browser dependency
  cannot land here silently.
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

## Versioning

`0.1.0`, and honestly so: the API is the extraction of a moving application
surface, not a stability promise. The package is marked `private` because
publishing to a registry is an explicit dispatch decision (see
`AGENTS.md`: never publish without current authorization naming the act) —
flipping `private` and choosing a scope owner is that decision, not a build
step.

## Layout

- `index.ts` — curated root exports; every module is also reachable as
  `@dclutch/sdk/<module>` and `@dclutch/sdk/generated/<module>`.
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
