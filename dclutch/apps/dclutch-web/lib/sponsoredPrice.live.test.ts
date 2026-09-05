import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import { DEVNET_DEPLOYMENT_V1 } from '@dclutch/sdk/deployments';
import { SolanaRpcClient } from '@dclutch/sdk/rpc';
import { readSponsoredPriceV1 } from './sourceProviderV1';

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' ? it : it.skip;

/**
 * The sponsored devnet SOL/USD feed, read through the Source family's own
 * decoder rather than through a second Pyth reader written in TypeScript.
 *
 * This is the number the founding wizard centres a band on. Until it existed
 * the wizard shipped `12000/18000` around `15000` -- a $150 SOL typed into a
 * runbook three months earlier -- and four devnet markets were founded
 * unfillable before anyone noticed a stale constant. Gated on
 * `DCLUTCH_LIVE_DEVNET=1`. Reads only.
 */
// Node has no `fetch` for a file URL, so the WASM arrives from disk -- the
// same bytes the digest canary in `loadSourceProviderWasmV1` checks.
const wasmPath = fileURLToPath(new URL('./generated/sourceProviderWasm/source_provider_bg.wasm', import.meta.url));
const transport = (async () => new Response(new Uint8Array(readFileSync(wasmPath)))) as unknown as typeof fetch;

const SPONSORED_SOL_USD = '7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE';
const PYTH_RECEIVER = 'rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ';

describe('live devnet sponsored price', () => {
  live('reads the sponsored SOL/USD PriceUpdateV2 exactly, and refuses the wrong owner', async () => {
    const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? DEVNET_DEPLOYMENT_V1.endpoint);
    const price = await readSponsoredPriceV1(client, {
      priceUpdateAddress: SPONSORED_SOL_USD,
      receiverProgram: PYTH_RECEIVER,
    }, transport);

    expect(price.address).toBe(SPONSORED_SOL_USD);
    expect(price.feedId).toMatch(/^[0-9a-f]{64}$/);
    // Shape, not a pinned quote: a price moves. What is pinned is that the
    // decode produced a positive price at Pyth's own negative exponent, with a
    // confidence and a publish time that are real, and an exact decimal that
    // is the two combined in integers rather than a double.
    expect(price.price > 0n).toBe(true);
    expect(price.exponent).toBeLessThan(0);
    expect(price.confidence >= 0n).toBe(true);
    expect(price.publishTimeUnixSeconds > 1_700_000_000n).toBe(true);
    expect(price.postedSlot).toMatch(/^\d+$/);
    expect(price.decimal).toMatch(/^\d+\.\d+$/);
    expect(price.decimal).toBe(
      `${price.price / 10n ** BigInt(-price.exponent)}.${(price.price % 10n ** BigInt(-price.exponent)).toString().padStart(-price.exponent, '0').replace(/0+$/, '')}`,
    );
    // SOL is not worth a dollar and is not worth a hundred thousand. A bound
    // this wide cannot pass on a misread exponent, which is the failure a
    // hand-written Pyth reader makes and the reason this one is not written
    // here at all.
    const dollars = Number(price.decimal);
    expect(dollars).toBeGreaterThan(1);
    expect(dollars).toBeLessThan(100_000);

    // The owner check is inside the WASM, so a well-formed account belonging
    // to something else is refused rather than decoded.
    await expect(readSponsoredPriceV1(client, {
      priceUpdateAddress: SPONSORED_SOL_USD,
      receiverProgram: DEVNET_DEPLOYMENT_V1.programs.core,
    }, transport)).rejects.toThrow(/not owned by the named receiver program/);
  }, 120_000);
});
