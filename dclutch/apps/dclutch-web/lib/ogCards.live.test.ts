import { renameSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import { DEVNET_DEPLOYMENT_V1 } from './deployments';
import { inspectMarketDetailV1 } from './marketDetail';
import { inspectMarketQuestionV1 } from './marketQuestion';
import { inspectMarketResolutionV1 } from './marketResolution';
import { SolanaRpcClient } from './rpc';
import { OG_CARD_FACTS_SCHEMA_V1, ogCardRowsV1, readOgCardFactsV1, readOgRegistryV1 } from '../scripts/og-cards.mjs';

/**
 * ONE READER AUTHORS THE SHARE-CARD FACTS AND CHECKS THEM.
 *
 * `fixtures/og-card-facts.devnet.json` carries the two facts that tell five
 * otherwise identical cards apart: each market's phase, and the instant it
 * settles or settled. Neither is editorial, so neither belongs in the registry;
 * both are chain facts, so a committed copy of them can go stale.
 *
 * A staged fact whose only witness is the tool that staged it is a fact nobody
 * is checking. So this file is BOTH halves: it reads the chain, and by default
 * it ASSERTS that every committed fact still agrees with what it read. Run it
 * with `DCLUTCH_OG_FACTS_WRITE=1` and it rewrites the fixture instead, which is
 * the authoring loop — and the rewrite goes through `ogCardRowsV1` first, so a
 * fixture that cannot produce cards is never written.
 *
 *   DCLUTCH_LIVE_DEVNET=1 DCLUTCH_OG_FACTS_WRITE=1 \
 *     npx vitest run --config vitest.config.ts lib/ogCards.live.test.ts
 *
 * A market that no longer reads — a closed cohort's — is dropped from the
 * fixture rather than recorded as an error. Its card keeps the lead alone,
 * which is the honest card for a market this cut cannot read.
 *
 * Devnet evidence. Not mainnet evidence.
 */
const FACTS_PATH = fileURLToPath(new URL('../fixtures/og-card-facts.devnet.json', import.meta.url));

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' ? it : it.skip;

type OgMarketFactsV1 = Readonly<{
  phase: string;
  settledAtUnixSeconds: string | null;
  windowEndUnixSeconds: string | null;
}>;

async function readMarketFactsV1(client: SolanaRpcClient, address: string): Promise<OgMarketFactsV1 | null> {
  const detail = await inspectMarketDetailV1(client, {
    coreProgramId: DEVNET_DEPLOYMENT_V1.programs.core,
    registryProgramId: DEVNET_DEPLOYMENT_V1.programs.registry,
    claimsProgramId: DEVNET_DEPLOYMENT_V1.programs.claims,
    custodyProgramId: DEVNET_DEPLOYMENT_V1.programs.custody,
    address,
  });
  if (detail.card.status !== 'decoded') return null;
  const card = detail.card;
  let windowEndUnixSeconds: string | null = null;
  try {
    const question = await inspectMarketQuestionV1(client, {
      registryProgramId: DEVNET_DEPLOYMENT_V1.programs.registry,
      address,
      productRecordId: card.identity.productRecordId,
      resolutionPolicyId: card.identity.resolutionPolicyId,
    });
    windowEndUnixSeconds = question.window === null ? null : question.window.endUnixSeconds.toString();
  } catch { windowEndUnixSeconds = null; }
  let settledAtUnixSeconds: string | null = null;
  if (card.settlement.status === 'terminal') {
    const resolution = await inspectMarketResolutionV1(client, {
      card, resolutionProgramId: DEVNET_DEPLOYMENT_V1.programs.resolution, floorSlot: card.observedSlot,
    });
    if (resolution.status === 'authenticated' && resolution.observation !== null) {
      settledAtUnixSeconds = resolution.observation.atUnixSeconds.toString();
    }
  }
  return Object.freeze({ phase: card.phase, settledAtUnixSeconds, windowEndUnixSeconds });
}

describe('live devnet: the share cards’ chain facts', () => {
  live('agrees with the chain about every market the committed fixture names', async () => {
    const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? DEVNET_DEPLOYMENT_V1.endpoint);
    const registry = readOgRegistryV1();
    const markets: Record<string, OgMarketFactsV1> = {};
    for (const address of Object.keys(registry.markets ?? {})) {
      const facts = await readMarketFactsV1(client, address);
      if (facts !== null) markets[address] = facts;
    }
    // A read that returned nothing at all is an endpoint problem, not a cohort
    // with no live markets, and must not be written over a good fixture.
    expect(Object.keys(markets).length, 'no registry market decoded, so this read proves nothing').toBeGreaterThan(0);

    const staged = { schema: OG_CARD_FACTS_SCHEMA_V1, cluster: 'devnet' as const, markets };
    // Cards must derive from what was read BEFORE anything is written or
    // compared: a fixture that cannot produce cards is the failure this whole
    // file exists to prevent.
    const rows = ogCardRowsV1(registry, staged);
    expect(rows.length).toBe(Object.keys(registry.markets ?? {}).length);

    if (process.env.DCLUTCH_OG_FACTS_WRITE === '1') {
      const body = `${JSON.stringify(staged, null, 2)}\n`;
      writeFileSync(`${FACTS_PATH}.staging`, body);
      renameSync(`${FACTS_PATH}.staging`, FACTS_PATH);
      return;
    }

    const committed = readOgCardFactsV1();
    expect(committed, 'no committed share-card facts; stage them with DCLUTCH_OG_FACTS_WRITE=1').not.toBeNull();
    // Field by field and market by market, so a stale card says WHICH fact
    // moved rather than that two documents differ.
    for (const [address, fresh] of Object.entries(markets)) {
      const held = committed!.markets?.[address];
      expect(held, `${address} reads on chain and the committed facts do not name it`).toBeDefined();
      expect(held!.phase, `${address} phase`).toBe(fresh.phase);
      expect(held!.settledAtUnixSeconds, `${address} settlement instant`).toBe(fresh.settledAtUnixSeconds);
      expect(held!.windowEndUnixSeconds, `${address} window end`).toBe(fresh.windowEndUnixSeconds);
    }
  });
});
