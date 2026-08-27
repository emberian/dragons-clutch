/**
 * @dclutch/sdk — the client surface of the dClutch protocol.
 *
 * Everything here was extracted from the web app's `lib/`, where it grew up
 * as the de facto SDK; the web app is now this package's first consumer. The
 * package is connection-agnostic by contract: nothing in it constructs an
 * RPC client, reads a wallet, or touches a browser global — the caller
 * supplies an endpoint (or a `SolanaRpcClient`) and signs its own
 * transactions. The generated modules under `lib/generated/` are emitted
 * from the protocol's Rust and Lean authorities and byte-gated by the
 * `abi:*:verify` scripts, which run inside `npm test`.
 *
 * This root re-exports the surfaces most clients start from. Every module is
 * also importable directly as `@dclutch/sdk/<module>` and
 * `@dclutch/sdk/generated/<module>`.
 */

// Reading the chain: the bounded, hostile-decoding RPC client and the
// account projections built on it.
export * from './lib/rpc';
export * from './lib/decoders';
export * from './lib/records';
export * from './lib/marketDiscovery';
export * from './lib/marketDetail';
export * from './lib/marketCoreV2';
export * from './lib/portfolio';
export * from './lib/activity';

// Refusals by name: band arithmetic over the registered code space.
export * from './lib/refusals';

// The Direct trading path: chain-derived inline buy/sell construction.
export * from './lib/directTransaction';
export * from './lib/directInlineV3';
export * from './lib/directCodec';

// Redemption: the Claims-role Custody replay the chain demands, then the
// terminal redemption wire.
export * from './lib/claimsCustodyReplay';

// Founding and infrastructure: the record graph a market stands on.
export * from './lib/coreFound';
export * from './lib/infrastructure';
export * from './lib/capabilityManifest';
export * from './lib/releaseRegistry';

// Local-validator conformance: verify a running successor byte-for-byte
// against the committed checkpoint fixture.
export * from './lib/localSuccessor';
