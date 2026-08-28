/**
 * @dclutch/sdk — the client surface of the dClutch protocol.
 *
 * Everything here was extracted from the web app's `lib/`, where it grew up
 * as the de facto SDK. The package is connection-agnostic by contract:
 * nothing in it constructs an
 * RPC client, reads a wallet, or touches a browser global — the caller
 * supplies an endpoint (or a `SolanaRpcClient`). Its public RPC surface is
 * read-only; a caller-specific durable workflow owns signing and the only
 * submission. The generated modules under `lib/generated/` are emitted
 * from the protocol's Rust and Lean authorities and byte-gated by the
 * `abi:*:verify` scripts, which run inside `npm test`.
 *
 * This root re-exports the surfaces most clients start from. Public subpaths
 * are also importable as `@dclutch/sdk/<module>`. A module that can construct
 * a packet is not public until its exact durable journal, acknowledgement, and
 * finalized poststate proof are part of the same caller-backed surface.
 */

// Reading the chain: the bounded, hostile-decoding RPC client and the
// account projections built on it.
export * from './lib/rpc';
export * from './lib/transactionReturnData';
export * from './lib/decoders';
export * from './lib/records';
export * from './lib/marketDiscovery';
export * from './lib/marketDetail';
export * from './lib/marketCoreV2';
export * from './lib/portfolio';
export * from './lib/activity';

// Refusals by name: band arithmetic over the registered code space.
export * from './lib/refusals';

// The funded failure walk: the permissionless deadline route, paid from the
// escrow the market funded at founding.
export * from './lib/failureWalk';

// Direct inspection: chain-derived route state, exact intent/arithmetic
// previews, and the walls between a market and an accepted trade caller.
export * from './lib/directTradeSpine';
export * from './lib/directMakerReplay';
export * from './lib/directTicket';
export * from './lib/directInlinePublicV3';
export * from './lib/directHotRouteManifest';
export * from './lib/solanaLimits';

// Redemption: the Claims-role Custody replay the chain demands, then the
// terminal redemption wire.
export * from './lib/claimsCustodyReplay';
export * from './lib/walletTerminalPayoutV3';

// Founding and infrastructure: the record graph a market stands on.
export * from './lib/coreFound';
export * from './lib/infrastructure';
export * from './lib/capabilityManifest';
export * from './lib/releaseRegistry';

// Local-validator conformance: verify a running successor byte-for-byte
// against the committed checkpoint fixture.
export * from './lib/localSuccessor';
