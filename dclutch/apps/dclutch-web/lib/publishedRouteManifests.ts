import { PublicKey } from '@solana/web3.js';

import manifest from '@/fixtures/published-route-manifests.json';

/**
 * Operator-published Direct Hot route manifests, baked per Market.
 *
 * The trade panel needs the operator's `dclutch-direct-hot-route-manifest-v3`
 * to prepare a real packet. A reader should not have to go find and paste it
 * for a market this build already knows: when the operator publishes the
 * manifest for a flagship market, it is committed here and the panel's
 * drawer opens pre-filled — still editable, still hostile-decoded on use,
 * because baking a manifest grants it no authority the pasted one lacks.
 *
 * Update only fixtures/published-route-manifests.json. An entry's value is
 * the manifest's exact JSON text; the key is the Market address.
 */

const SCHEMA = 'dclutch-published-route-manifests-v1';

type PublishedRouteManifestsV1 = Readonly<{
  schema: typeof SCHEMA;
  manifests: Readonly<Record<string, string>>;
}>;

function parse(value: unknown): PublishedRouteManifestsV1 {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new Error('published route manifests must be one object');
  const record = value as Record<string, unknown>;
  if (record.schema !== SCHEMA) throw new Error('published route manifests have another schema');
  const raw = record.manifests;
  if (raw === null || typeof raw !== 'object' || Array.isArray(raw)) throw new Error('published route manifests must map Market addresses to manifest text');
  const manifests: Record<string, string> = {};
  for (const [address, text] of Object.entries(raw as Record<string, unknown>)) {
    const canonical = new PublicKey(address).toBase58();
    if (canonical !== address) throw new Error(`published route manifest key ${address} is not canonical base58`);
    if (typeof text !== 'string' || text.trim() === '') throw new Error(`published route manifest for ${address} is not nonempty text`);
    manifests[address] = text;
  }
  return Object.freeze({ schema: SCHEMA, manifests: Object.freeze(manifests) });
}

const PUBLISHED = parse(manifest);

/** The baked operator manifest for one Market, or null when none is published in this build. */
export function publishedDirectRouteManifestV1(marketAddress: string): string | null {
  return PUBLISHED.manifests[marketAddress] ?? null;
}
