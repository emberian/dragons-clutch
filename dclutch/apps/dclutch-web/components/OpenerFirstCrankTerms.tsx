'use client';

import { useCallback, useEffect, useState } from 'react';

import {
  COMPACTION_CRANK_REWARD_LAMPORTS_V1,
  OPENER_ACCOUNT_WIDTHS_V1,
  lamportsAsSolV1,
  openerFirstCrankV1,
  type OpenerFirstCrankV1,
} from '@dclutch/sdk/openerTerms';
import { SolanaRpcClient } from '@dclutch/sdk/rpc';

type State =
  | Readonly<{ kind: 'loading' | 'refused'; message: string }>
  | Readonly<{ kind: 'ready'; plan: OpenerFirstCrankV1 }>;

/**
 * The market's terms sentence for the crank-first order.
 *
 * RULED 2026-09-04 (C-11 D1 item 2): the crank-first order stands, and the
 * market's terms state it. Opening costs the opener the first crank, and a
 * market whose escrow is compacted exactly once never repays them.
 *
 * The number is read off the cluster this page is pointed at, four
 * `getMinimumBalanceForRentExemption` calls, and run through the kernel's own
 * order -- because rent is a cluster parameter that moves. Devnet went 6,333 to
 * 5,080 lamports a byte inside cohort-15, a fifth off this figure in a day, so
 * a page that quoted a number would have been quietly wrong about what it was
 * charging a founder. When the reads refuse, this says so and states no figure.
 */
export default function OpenerFirstCrankTerms({
  endpoint,
  outcomeCount,
  heading,
}: Readonly<{
  endpoint: string;
  outcomeCount: number;
  heading: string;
}>) {
  const [state, setState] = useState<State>({ kind: 'loading', message: 'Reading this cluster’s rent minimums…' });

  const read = useCallback(async () => {
    setState({ kind: 'loading', message: 'Reading this cluster’s rent minimums…' });
    const widths = OPENER_ACCOUNT_WIDTHS_V1;
    const client = new SolanaRpcClient(endpoint);
    const needed = [
      widths.claimCheck,
      widths.claimCheckEscrow,
      widths.tokenAccount,
      widths.admission,
      widths.positionHeader + widths.positionPerOutcome * outcomeCount,
    ];
    try {
      const observed = new Map<number, bigint>();
      for (const bytes of needed) {
        if (observed.has(bytes)) continue;
        const observation = await client.minimumBalanceForRentExemption(bytes);
        observed.set(bytes, BigInt(observation.lamports));
      }
      const rentFor = (bytes: number) => {
        const lamports = observed.get(bytes);
        if (lamports === undefined) throw new Error(`no rent minimum was read for ${bytes} bytes`);
        return lamports;
      };
      setState({ kind: 'ready', plan: openerFirstCrankV1({ outcomeCount, rentFor }) });
    } catch (error) {
      setState({
        kind: 'refused',
        message: `The cluster did not answer for its rent minimums, so no figure is stated here: ${error instanceof Error ? error.message : 'no reason was given'}.`,
      });
    }
  }, [endpoint, outcomeCount]);

  // Deferred out of the effect body for the same reason the retirement drawer
  // beside it defers: a synchronous `setState` in an effect is a cascading
  // render, and the read is asynchronous anyway.
  useEffect(() => {
    let cancelled = false;
    queueMicrotask(() => {
      if (!cancelled) void read();
    });
    return () => { cancelled = true; };
  }, [read]);

  return <section className="opener-terms" aria-label="What opening this market costs the opener">
    <h3 className="detail-subhead">{heading}</h3>
    <p>
      Opening a claim-check escrow here costs the opener the first crank. The opener advances rent for
      the escrow record and its token vault; the first permissionless compaction sweeps the Position and
      the admission record, pays the new claim check&apos;s own rent, pays <strong>the cranker before the
      opener</strong>, and repays the opener only out of what is left. A market whose escrow is compacted
      exactly once never repays its opener in full.
    </p>
    {state.kind !== 'ready'
      ? <p className={state.kind === 'refused' ? 'market-refusal' : 'direct-status'} aria-live="polite">{state.message}</p>
      : <>
        <dl className="detail-facts">
          <div><dt>The opener advances</dt><dd>{lamportsAsSolV1(state.plan.openerOutlay)} SOL · {state.plan.openerOutlay.toString()} lamports</dd></div>
          <div><dt>The first crank repays</dt><dd>{lamportsAsSolV1(state.plan.openerRepayment)} SOL</dd></div>
          <div><dt>Still owed after it</dt><dd><strong>{lamportsAsSolV1(state.plan.openerStillOwed)} SOL</strong> · {state.plan.openerStillOwed.toString()} lamports</dd></div>
          <div><dt>The cranker is paid</dt><dd>{lamportsAsSolV1(state.plan.crankReward)} SOL, first</dd></div>
        </dl>
        <p className="direct-status">
          Read from this cluster&apos;s own rent minimums at {outcomeCount} outcomes, not quoted: rent is a
          cluster parameter and devnet moved it by a fifth inside one cohort. A market compacted more than
          once repays the opener progressively; the cap on one crank&apos;s reward is{' '}
          {lamportsAsSolV1(COMPACTION_CRANK_REWARD_LAMPORTS_V1)} SOL and it is a ceiling on a residual, never
          a demand — a thin sweep pays a thin reward rather than refusing, because a crank that could refuse
          for lack of funds is a crank nobody turns.
        </p>
      </>}
  </section>;
}
