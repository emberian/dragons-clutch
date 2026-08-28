/**
 * The RPC endpoint every workspace seeds its editable endpoint field with.
 *
 * DEPLOY-1 pointed the app at PUBLIC devnet: the protocol's durable substrate
 * lives there, and visitors query from their own IPs, so the public endpoint's
 * per-IP rate limits are per-visitor rather than shared through one proxy. The
 * field stays editable in every surface for anyone running a local validator
 * or their own paid endpoint.
 *
 * `NEXT_PUBLIC_DCLUTCH_RPC` overrides the default at build time. A static
 * export BAKES that value into the public bundle, so it must only ever carry a
 * URL that is deliberately public — never an ops key. (If visitor-grade paid
 * RPC is wanted later, that is a separate origin-locked key minted for the
 * site's own domain, chosen to be public.)
 */
export const DEFAULT_RPC_ENDPOINT_V1: string =
  process.env.NEXT_PUBLIC_DCLUTCH_RPC ?? 'https://api.devnet.solana.com';

/**
 * Cluster identity by genesis hash — the chain's OWN answer, never the URL's.
 *
 * The devnet hash is the one `dclutch_pyth_svm::devnet::DEVNET_CLUSTER_ID_V1`
 * binds and the campaign driver's acknowledgment names; mainnet-beta's is
 * listed so a mainnet chain is NAMED as itself rather than shown as merely
 * unrecognized.
 */
export const KNOWN_GENESIS_HASHES_V1: Readonly<Record<string, string>> = Object.freeze({
  EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG: 'devnet',
  '5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d': 'mainnet-beta',
});

/** Name the connected cluster from its reported genesis hash. */
export function clusterNameV1(genesisHash: string): string {
  return KNOWN_GENESIS_HASHES_V1[genesisHash] ?? 'unrecognized cluster';
}
