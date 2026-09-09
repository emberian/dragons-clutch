'use client';

import { useCallback, useEffect, useState } from 'react';

import {
  COMPACTION_CRANK_REWARD_LAMPORTS_V1,
  OPENER_ACCOUNT_WIDTHS_V1,
  fundedRentMinimumV1,
  fundedRentRateFromMinimumV1,
  lamportsAsSolV1,
  openerFirstCrankAtFundedRateV1,
  type OpenerFirstCrankV1,
} from '@dclutch/sdk/openerTerms';
import { SolanaRpcClient } from '@dclutch/sdk/rpc';

type State =
  | Readonly<{ kind: 'loading' | 'refused'; message: string }>
  | Readonly<{ kind: 'ready'; plan: OpenerFirstCrankV1; rate: bigint; recorded: boolean }>;

/**
 * The market's terms sentence for the crank-first order.
 *
 * RULED 2026-09-04 (C-11 D1 item 2): the crank-first order stands, and the
 * market's terms state it. Opening costs the opener the first crank, and a
 * market whose escrow is compacted exactly once never repays them.
 *
 * # One rate, and it is stated
 *
 * The figure is priced from a single lamports-per-byte RATE, the way the
 * protocol itself prices: `funded_rent_minimum_v2` is affine in the length, so
 * one rate prices every account a founding creates, and one `u32` is what a
 * founding persists in its capability funding ledger.
 *
 * A market that already exists passes its RECORDED rate as `fundedRentRate`,
 * and the number then describes that market rather than this cluster: its
 * accounts hold what its own founding funded them with. A founding surface
 * passes nothing, and the rate is derived from the cluster this page is pointed
 * at, exactly as `derive_funded_rent_rate_v2` derives the rate the founding is
 * about to record -- two readings, cross-checked, and a cluster whose rent is
 * not affine is refused by name rather than approximated.
 *
 * Either way the rate is PRINTED, because it is the whole provenance of the
 * number beside it. Devnet went 6,333 to 5,080 lamports a byte inside
 * cohort-15, a fifth off this figure in a day, so a page that quoted a number
 * without its rate would have been quietly wrong about what it was charging a
 * founder. When the reads refuse, this says so and states no figure.
 */
export default function OpenerFirstCrankTerms({
  endpoint,
  outcomeCount,
  heading,
  fundedRentRate,
}: Readonly<{
  endpoint: string;
  outcomeCount: number;
  heading: string;
  /**
   * The lamports-per-byte rate this market's founding recorded, when there is
   * a market. Absent on the founding surface, where no rate has been recorded
   * yet and the cluster's own is what one will be derived from.
   */
  fundedRentRate?: bigint;
}>) {
  const [state, setState] = useState<State>({ kind: 'loading', message: 'Reading account storage costs…' });

  const read = useCallback(async () => {
    if (fundedRentRate !== undefined) {
      setState({
        kind: 'ready',
        plan: openerFirstCrankAtFundedRateV1({ outcomeCount, fundedRentRate }),
        rate: fundedRentRate,
        recorded: true,
      });
      return;
    }
    setState({ kind: 'loading', message: 'Reading account storage costs…' });
    const widths = OPENER_ACCOUNT_WIDTHS_V1;
    const client = new SolanaRpcClient(endpoint);
    try {
      // TWO readings, which is what pins an affine function. The zero-length
      // one gives the rate; the claim check's own width has to agree with it
      // exactly, and a cluster that does not is refused rather than averaged --
      // `derive_funded_rent_rate_v2`'s discipline, and for its reason: an
      // approximated rate is a recorded number that silently prices some other
      // account wrong.
      const zero = BigInt((await client.minimumBalanceForRentExemption(0)).lamports);
      const rate = fundedRentRateFromMinimumV1(zero, 0);
      const witness = BigInt((await client.minimumBalanceForRentExemption(widths.claimCheck)).lamports);
      if (fundedRentMinimumV1(rate, widths.claimCheck) !== witness) {
        throw new Error(
          `this cluster’s rent is not affine in the account length: ${rate} lamports a byte prices ${widths.claimCheck} bytes at ${fundedRentMinimumV1(rate, widths.claimCheck)} and the cluster answered ${witness}`,
        );
      }
      setState({
        kind: 'ready',
        plan: openerFirstCrankAtFundedRateV1({ outcomeCount, fundedRentRate: rate }),
        rate,
        recorded: false,
      });
    } catch (error) {
      setState({
        kind: 'refused',
        message: `Unable to calculate storage costs: ${error instanceof Error ? error.message : 'no reason was given'}.`,
      });
    }
  }, [endpoint, outcomeCount, fundedRentRate]);

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
      The opener pays the storage deposit for the escrow record and token vault. When anyone compacts
      the escrow, recovered deposits fund the new claim check and pay <strong>the cranker before the
      opener</strong>. The opener receives the remainder. One compaction does not repay the opener in full.
    </p>
    {state.kind !== 'ready'
      ? <p className={state.kind === 'refused' ? 'market-refusal' : 'direct-status'} aria-live="polite">{state.message}</p>
      : <>
        <dl className="detail-facts">
          <div><dt>The opener advances</dt><dd>{lamportsAsSolV1(state.plan.openerOutlay)} SOL · {state.plan.openerOutlay.toString()} lamports</dd></div>
          <div><dt>First compaction repayment</dt><dd>{lamportsAsSolV1(state.plan.openerRepayment)} SOL</dd></div>
          <div><dt>Still owed to the opener</dt><dd><strong>{lamportsAsSolV1(state.plan.openerStillOwed)} SOL</strong> · {state.plan.openerStillOwed.toString()} lamports</dd></div>
          <div><dt>Cranker reward</dt><dd>{lamportsAsSolV1(state.plan.crankReward)} SOL, first</dd></div>
          <div><dt>Priced at</dt><dd>{state.rate.toString()} lamports a byte · {state.recorded ? 'this market’s recorded founding rate' : 'current cluster rate'}</dd></div>
        </dl>
        <p className="direct-status">
          Calculated for {outcomeCount} outcomes at the rate above. Further compactions can repay the
          opener progressively. Each crank reward is capped at{' '}
          {lamportsAsSolV1(COMPACTION_CRANK_REWARD_LAMPORTS_V1)} SOL and limited to the available funds.
          A smaller balance reduces the reward; it does not block compaction.
        </p>
      </>}
  </section>;
}
