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
 * This root re-exports the surfaces most clients start from. Every module is
 * also importable as `@dclutch/sdk/<module>`. Submission is one bounded
 * primitive, `SolanaRpcClient.sendRawTransaction`, and every caller that
 * reaches it owns a durable journal around it (`walletHandoff` for a browser
 * wallet; the CLI's own transport for a terminal).
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
export * from './lib/bundleExposure';
export * from './lib/activity';
export * from './lib/deployments';
export * from './lib/operatorSurface';

// The executable capability model: what each protocol act is, and the rules
// that derive its venue, authority and walls from a client's own evidence.
// A consumer supplies the evidence; nothing here carries a status a hand
// could set.
export * from './lib/capabilityModel';

// Refusals by name: band arithmetic over the registered code space.
export * from './lib/refusals';

// The funded failure walk: the permissionless deadline route, paid from the
// escrow the market funded at founding.
export * from './lib/failureWalk';

// Direct inspection: chain-derived route state, exact intent/arithmetic
// previews, and the walls between a market and an accepted trade caller.
export * from './lib/directTradeSpine';
export * from './lib/directParticipant';
export * from './lib/directMakerReplay';
export * from './lib/directTicket';
export * from './lib/directOfferAuthoring';
export * from './lib/directInlineV3';
export * from './lib/directHotRouteManifest';
export * from './lib/directWalletPreparationV1';
export * from './lib/solanaLimits';

// Redemption: the Claims-role Custody replay the chain demands, then the
// terminal redemption wire.
export * from './lib/claimsCustodyReplay';
export * from './lib/walletTerminalPayoutV3';
export * from './lib/resolutionCertificateV2';
export * from './lib/aggregateRetirement';

// Founding and infrastructure: the record graph a market stands on.
export * from './lib/coreFound';
export * from './lib/splineProductAuthoring';
export * from './lib/infrastructure';
export * from './lib/capabilityManifest';
export * from './lib/founding/principalCapacity';
export * from './lib/releaseRegistry';
export * from './lib/rationalTerminalChainV4';
