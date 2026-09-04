'use client';

import { useCallback, useEffect, useState } from 'react';
import {
  inspectAggregateRetirementV1,
  type AggregateRetirementInspectionV1,
} from '@dclutch/sdk/aggregateRetirement';

import Anchor from '@/components/Anchor';
import { type MarketCorePhaseV2 } from '@/lib/marketCoreV2';
import { SolanaRpcClient } from '@/lib/rpc';

type State =
  | Readonly<{ kind: 'loading' | 'refused'; message: string }>
  | Readonly<{ kind: 'ready'; inspection: AggregateRetirementInspectionV1 }>;

const errorMessage = (error: unknown) => error instanceof Error
  ? error.message
  : 'the retirement read refused without a usable reason';

const orderedSteps = Object.freeze([
  ['prepare', 'Prove every Claims liability is zero and persist ClaimsClosed'],
  ['close-vault', 'Close the empty HoardPrincipal vault'],
  ['close-replay', 'Close the normal Custody replay'],
  ['finish', 'Reauthenticate the original bundle and close Core and Rent'],
] as const);

export default function AggregateRetirementStatus({
  endpoint,
  coreProgramId,
  claimsProgramId,
  marketAddress,
  marketPhase,
  marketGeneration,
  minimumContextSlot,
}: Readonly<{
  endpoint: string;
  coreProgramId: string;
  claimsProgramId: string;
  marketAddress: string;
  marketPhase: MarketCorePhaseV2;
  marketGeneration: string;
  minimumContextSlot: string;
}>) {
  const [state, setState] = useState<State>({ kind: 'loading', message: 'Reading the derived Claims aggregate or Core retirement checkpoint at the Market floor…' });

  const inspect = useCallback(async () => {
    setState({ kind: 'loading', message: 'Reading the derived Claims aggregate or Core retirement checkpoint at the Market floor…' });
    try {
      const inspection = await inspectAggregateRetirementV1(new SolanaRpcClient(endpoint), {
        coreProgramId,
        claimsProgramId,
        marketAddress,
        marketPhase,
        marketGeneration,
        minimumContextSlot,
      });
      setState({ kind: 'ready', inspection });
    } catch (error) {
      setState({ kind: 'refused', message: `Refused: ${errorMessage(error)}` });
    }
  }, [claimsProgramId, coreProgramId, endpoint, marketAddress, marketGeneration, marketPhase, minimumContextSlot]);

  useEffect(() => {
    let cancelled = false;
    queueMicrotask(() => {
      if (!cancelled) void inspect();
    });
    return () => { cancelled = true; };
  }, [inspect]);

  const inspection = state.kind === 'ready' ? state.inspection : null;
  const hostile = inspection?.status === 'refused' || inspection?.status === 'blocked-liabilities';

  /**
   * The summary line, read off the state rather than written.
   *
   * This section is a DISCLOSURE now, and a disclosure's summary is the only
   * thing a reader sees until they open it — so a fixed "Retirement checkpoint"
   * would make every market's fold look identical while their checkpoints are
   * not. The status and the next durable step both come from the inspection,
   * and before it returns the line says which of the two silences this is: a
   * read still running, or a read that refused.
   */
  const summary = inspection !== null
    ? `Retirement checkpoint · ${inspection.status.replaceAll('-', ' ')} · next durable step ${inspection.nextStep}`
    : state.kind === 'refused'
      ? 'Retirement checkpoint · the read refused'
      : 'Retirement checkpoint · reading';

  return <details className="market-detail-drawer retirement-drawer">
    <summary>{summary}</summary>
    <div className="market-detail-drawer-body">
    <p>You can see whether this Market has reached the packet-bounded retirement waist and, if it has, which durable step comes next. The account is derived from this Market and decoded from the Rust-owned generated ABI. Browser storage is never treated as progress.</p>
    <div className="direct-actions"><button type="button" onClick={() => void inspect()} disabled={state.kind === 'loading'}>{state.kind === 'loading' ? 'Reading…' : 'Re-read retirement'}</button></div>

    {inspection === null
      ? <p className={state.kind === 'refused' ? 'market-refusal' : 'direct-status'} aria-live="polite">{state.kind === 'ready' ? 'The retirement read returned no inspection.' : state.message}</p>
      : <>
        <p className={hostile ? 'market-refusal' : 'market-capability-refusal'} aria-live="polite"><span>{inspection.status.replaceAll('-', ' ')}</span>{inspection.reason}</p>
        <dl className="detail-facts">
          <div><dt>Derived aggregate / checkpoint</dt><dd title={inspection.aggregateAddress}>{inspection.aggregateAddress}</dd></div>
          <div><dt>Finalized observed slot</dt><dd>{inspection.observedSlot}</dd></div>
          <div><dt>Next durable step</dt><dd>{inspection.nextStep}</dd></div>
          <div><dt>Nonzero claim entries</dt><dd>{inspection.nonzeroClaimCount ?? 'not asserted by this state'}</dd></div>
          {inspection.checkpoint !== null && <>
            <div><dt>Persisted phase</dt><dd>{inspection.checkpoint.phase}</dd></div>
            <div><dt>Phase revision</dt><dd>{inspection.checkpoint.phaseRevision}</dd></div>
            <div><dt>Claims refund · lamports</dt><dd>{inspection.checkpoint.claimsRefundLamports}</dd></div>
            <div><dt>Custody refund · lamports</dt><dd>{inspection.checkpoint.custodyRefundLamports}</dd></div>
            <div><dt>Original bundle digest</dt><dd title={inspection.checkpoint.bundleDigest}>{inspection.checkpoint.bundleDigest}</dd></div>
          </>}
        </dl>
      </>}

    <ol className="market-bindings">
      {orderedSteps.map(([step, detail]) => <li key={step} className={inspection?.nextStep === step ? 'check-pass' : ''}>
        <span aria-hidden="true">{inspection?.nextStep === step ? '→' : '·'}</span>
        <div><strong>{step}</strong><small>{detail}</small></div>
      </li>)}
    </ol>
    <div className="direct-actions">
      <button type="button" disabled>Retirement unavailable in this browser</button>
      <Anchor href="/operate">Inspect the operator boundary →</Anchor>
    </div>
    <p className="direct-status">You still need a checked release that selects this exact route and the Rust-authored four-step campaign with one durable crash journal per mutation. A local-validator execution is not devnet execution. This page never reconstructs the original bundle, opens a wallet, signs, or submits.</p>
    </div>
  </details>;
}
