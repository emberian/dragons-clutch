'use client';

import { useEffect, useState } from 'react';

import Anchor from '@/components/Anchor';
import { useDeploymentV1 } from '@/lib/deploymentStore';
import { decodeMarketCoreStateV2, type MarketCorePhaseV2 } from '@dclutch/sdk/marketCoreV2';
import { PUBLIC_DEVNET_CUT_V1, publicCutMarketHrefV1 } from '@dclutch/sdk/publicCutStaging';
import { marketEditorialV1 } from '@dclutch/sdk/marketRegistry';
import { SolanaRpcClient } from '@dclutch/sdk/rpc';

/**
 * THE FRONT DOOR'S FEATURED-MARKET DISCLOSURE, read instead of written.
 *
 * The aside said "the first market is open" as a fixed word, and the word was
 * a promise about a chain fact that nobody was checking. It is already the
 * second time this exact shape has bitten this page: it went on saying no
 * market was open after one was, and the fix then was to read the public cut
 * rather than a literal. The cut names the ADDRESS. It cannot name the phase,
 * because a phase moves — and a resolution moves it the same afternoon a fill
 * lands.
 *
 * So the phase and OWNER come off the market's own Core account. Two round
 * trips, the cheapest read in the app: one finalized floor and one account.
 * The owner check is not optional. Cohort-17 left the cohort-16 market in the
 * public-cut fixture, so a link which merely had a valid address was pointing
 * readers at a retired cohort while the front door called it current.
 *
 * A cut/deployment mismatch is a useful historical record, but it is not an
 * entrance to the active cohort. The page says exactly that and sends readers
 * to the deployment's live market scan. A phase that will not read says no
 * verb about a state it did not check.
 */

/** What each phase means for what a stranger can do on the front door. */
const FRONT_DOOR_PHASE_V1: Readonly<Record<MarketCorePhaseV2, string>> = Object.freeze({
  Founding: 'still being set up',
  Open: 'open',
  Terminal: 'resolved — its answer is in',
  Retiring: 'winding down',
  Retired: 'finished',
});

export type FeaturedMarketStandingV1 =
  | Readonly<{ kind: 'reading' | 'unread' | 'other-cohort' }>
  | Readonly<{ kind: 'read'; phase: MarketCorePhaseV2 }>;

export function frontDoorPhraseV1(standing: FeaturedMarketStandingV1): string | null {
  return standing.kind === 'read' ? FRONT_DOOR_PHASE_V1[standing.phase] : null;
}

export default function FeaturedMarketStanding() {
  const deployment = useDeploymentV1();
  const market = PUBLIC_DEVNET_CUT_V1.market;
  const [standing, setStanding] = useState<FeaturedMarketStandingV1>({ kind: 'reading' });

  useEffect(() => {
    if (market === null) return undefined;
    let cancelled = false;
    (async () => {
      try {
        const client = new SolanaRpcClient(deployment.endpoint);
        const floor = await client.finalizedSlot();
        const observation = await client.accountInfo(market, floor);
        const account = observation.account;
        if (account === null) {
          if (!cancelled) setStanding({ kind: 'unread' });
          return;
        }
        if (account.owner !== deployment.programs.core) {
          if (!cancelled) setStanding({ kind: 'other-cohort' });
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
    return <>No featured market has been staged for this public build. Browse the <Anchor href="/markets">market list</Anchor> to read the selected deployment.</>;
  }
  const link = <Anchor href={publicCutMarketHrefV1(PUBLIC_DEVNET_CUT_V1)}>
    {title === null ? 'the one they run' : title}
  </Anchor>;
  if (standing.kind === 'other-cohort') {
    return <>This build&apos;s featured record, {link}, belongs to another cohort. It is not presented as an active market; <Anchor href="/markets">browse the selected deployment</Anchor> for markets its Core owns.</>;
  }
  const phrase = frontDoorPhraseV1(standing);
  if (phrase === null) {
    return <>This build names {link} as a featured record. Its cohort link and state are read from the chain when this page loads; <Anchor href="/markets">browse the selected deployment</Anchor> in the meantime.</>;
  }
  if ('phase' in standing && (standing.phase === 'Retiring' || standing.phase === 'Retired')) {
    return <>The featured market, {link}, is <strong>{phrase}</strong>. <Anchor href="/markets">Browse the selected deployment</Anchor> for a market that is open.</>;
  }
  return <>The featured market is {link} — <strong>{phrase}</strong>, read from its own record.</>;
}
