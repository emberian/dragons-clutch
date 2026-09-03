'use client';

import { useEffect, useState } from 'react';

import Anchor from '@/components/Anchor';
import { useDeploymentV1 } from '@/lib/deploymentStore';
import { decodeMarketCoreStateV2, type MarketCorePhaseV2 } from '@/lib/marketCoreV2';
import { PUBLIC_DEVNET_CUT_V1, publicCutMarketHrefV1 } from '@/lib/publicCutStaging';
import { marketEditorialV1 } from '@/lib/marketRegistry';
import { SolanaRpcClient } from '@/lib/rpc';

/**
 * THE FRONT DOOR'S ONE DATED SENTENCE, read instead of written.
 *
 * The aside said "the first market is open" as a fixed word, and the word was
 * a promise about a chain fact that nobody was checking. It is already the
 * second time this exact shape has bitten this page: it went on saying no
 * market was open after one was, and the fix then was to read the public cut
 * rather than a literal. The cut names the ADDRESS. It cannot name the phase,
 * because a phase moves — and a resolution moves it the same afternoon a fill
 * lands.
 *
 * So the phase comes off the market's own Core account. Two round trips, the
 * cheapest read in the app: one finalized floor and one account. A phase that
 * will not read leaves the sentence with the cut's own claim and no verb about
 * a state it did not check, which is the honest degradation — never a guess,
 * and never a dash where a link belongs.
 */

/** What each phase means for what a stranger can do on the front door. */
const FRONT_DOOR_PHASE_V1: Readonly<Record<MarketCorePhaseV2, string>> = Object.freeze({
  Founding: 'still being set up',
  Open: 'open',
  Terminal: 'resolved — its answer is in',
  Retiring: 'winding down',
  Retired: 'finished',
});

type Standing =
  | Readonly<{ kind: 'reading' | 'unread' }>
  | Readonly<{ kind: 'read'; phase: MarketCorePhaseV2 }>;

export function frontDoorPhraseV1(standing: Standing): string | null {
  return standing.kind === 'read' ? FRONT_DOOR_PHASE_V1[standing.phase] : null;
}

export default function FeaturedMarketStanding() {
  const deployment = useDeploymentV1();
  const market = PUBLIC_DEVNET_CUT_V1.market;
  const [standing, setStanding] = useState<Standing>({ kind: 'reading' });

  useEffect(() => {
    if (market === null) return undefined;
    let cancelled = false;
    (async () => {
      try {
        const client = new SolanaRpcClient(deployment.endpoint);
        const floor = await client.finalizedSlot();
        const observation = await client.accountInfo(market, floor);
        const account = observation.account;
        if (account === null || account.owner !== deployment.programs.core) {
          if (!cancelled) setStanding({ kind: 'unread' });
          return;
        }
        const state = decodeMarketCoreStateV2(market, account.data);
        if (!cancelled) setStanding({ kind: 'read', phase: state.phase });
      } catch {
        if (!cancelled) setStanding({ kind: 'unread' });
      }
    })();
    return () => { cancelled = true; };
  }, [deployment, market]);

  // The registry stopped writing titles for live markets, because
  // `derivedTitleV1` writes a better one off the market's own partition -- and
  // this component has no chain read for that and should not grow one for a
  // link label. So the link takes the COORDINATE's common name, which is the
  // one editorial field that survives a re-founding, and falls back to the
  // generic phrase only when the registry knows the market by no name at all.
  const editorial = market === null ? null : marketEditorialV1(market);
  const title = editorial?.title ?? editorial?.coordinate?.label ?? null;
  if (market === null) {
    return <>and the first markets are being set up.</>;
  }
  const phrase = frontDoorPhraseV1(standing);
  const link = <Anchor href={publicCutMarketHrefV1(PUBLIC_DEVNET_CUT_V1)}>
    {title === null ? 'the one they run' : title}
  </Anchor>;
  // Both arms carry the same clause, because the sentence is about the market
  // either way; only the VERB is a chain fact, and only the verb waits.
  return phrase === null
    ? <>and the first market is {link}. What state it is in is read on its own page.</>
    : <>and the first market is {link} — <strong>{phrase}</strong> right now, read from its own record.</>;
}
