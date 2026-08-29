'use client';

import { useState } from 'react';

import WalletDirectory, { useWalletDirectoryV1 } from '@/components/WalletDirectory';
import {
  inspectDirectParticipantReadinessV1,
  type DirectParticipantReadinessV1,
} from '@/lib/directParticipant';
import { SolanaRpcClient } from '@dclutch/sdk/rpc';

/**
 * The joining face of one Market: does the connected wallet hold a Position
 * here, and if not, exactly what joining creates and how to do it.
 *
 * Everything shown is chain-read: the readiness inspection derives the
 * participant's account addresses from the Market and the wallet, then reads
 * them at one finalized floor. The panel never invents a probability, a
 * balance, or a state.
 *
 * Admission itself is not composed in the browser. The admission transaction
 * (Position + admission evidence + the seeded collateral account, with exact
 * rent top-ups) is built by the operator toolchain that the protocol's own
 * lifecycle tests drive, and it needs the position owner's signature over a
 * frame the browser cannot yet assemble byte-exactly. This panel says that
 * plainly and hands the reader the exact command instead of a button that
 * could not tell the truth.
 */

type InspectionState =
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'working' }>
  | Readonly<{ kind: 'done'; readiness: DirectParticipantReadinessV1 }>
  | Readonly<{ kind: 'refused'; reason: string }>;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'the read refused without a usable reason';
}

export const JOIN_MISSING_MEANING_V1: Readonly<Record<string, string>> = Object.freeze({
  'Position and admission':
    'your Position account and its admission evidence — the accounts that make you a participant',
  'collateral account':
    'your market collateral account — the token account your deposits and trade settlements move through',
});

/** Joining a resolved or retiring market buys nothing; say so instead of offering it. */
export function joiningClosedForPhaseV1(marketPhase: string): boolean {
  const phase = marketPhase.toLowerCase();
  return phase.includes('terminal') || phase.includes('retir');
}

/** Pure rendering of one wallet's standing on one Market — testable as props → markup. */
export function JoinStanding({
  readiness,
  marketPhase,
  walletAddress,
}: Readonly<{
  readiness: DirectParticipantReadinessV1;
  marketPhase: string;
  walletAddress: string;
}>) {
  if (readiness.status === 'refused') {
    return <p className="market-refusal">Refused: {readiness.reason}</p>;
  }
  if (readiness.status === 'ready') {
    return <>
      <p className="direct-status">You are a participant on this market. Read at finalized slot {readiness.observedSlot}.</p>
      <dl className="detail-facts">
        <div><dt>Position</dt><dd><code>{readiness.coordinates.position}</code></dd></div>
        <div><dt>Position revision</dt><dd>{String(readiness.positionRevision)}</dd></div>
        <div><dt>Claim balances (atoms)</dt><dd>{readiness.positionBalances.map((balance) => String(balance)).join(' · ')}</dd></div>
        <div><dt>Collateral account</dt><dd><code>{readiness.coordinates.collateral}</code></dd></div>
        <div><dt>Collateral (atoms)</dt><dd>{String(readiness.collateralAtoms)}</dd></div>
        <div><dt>Spendable (atoms)</dt><dd>{String(readiness.spendableCollateralAtoms)}</dd></div>
      </dl>
      <p className="direct-status">The trade panel below trades against exactly these accounts.</p>
    </>;
  }
  return <>
    <p className="direct-status">This wallet is not a participant here yet. Read at finalized slot {readiness.observedSlot}.</p>
    <p className="detail-subhead">Joining creates, at addresses already derived from this Market and your wallet:</p>
    <ul className="market-bindings">
      {readiness.missing.map((item) => <li key={item}>
        <strong>{item}</strong> — {JOIN_MISSING_MEANING_V1[item] ?? 'a required participant account'}
      </li>)}
    </ul>
    <dl className="detail-facts">
      <div><dt>Your Position will live at</dt><dd><code>{readiness.coordinates.position}</code></dd></div>
      <div><dt>Your collateral account</dt><dd><code>{readiness.coordinates.collateral}</code></dd></div>
    </dl>
    {joiningClosedForPhaseV1(marketPhase)
      ? <p className="market-refusal">This market has already resolved, so joining it now would buy nothing: no further trades or claims are possible on a terminal market.</p>
      : <>
        <p className="detail-subhead">How to join</p>
        <p className="direct-status">Joining runs through the <code>dclutch</code> command line today. Your own keys sign; the transaction is built and checked by the same code the protocol&apos;s lifecycle tests drive, and nothing is sent until you add <code>--execute</code>. You will need the market&apos;s campaign documents — published alongside a public market&apos;s evidence, or written by your own bootstrap run on a local validator.</p>
        <pre className="trade-v3-bytes">{`dclutch join \\
  --plan <campaign plan.json> \\
  --campaign-evidence <campaign evidence.json> \\
  --keypair <your keypair.json> \\
  --output <admission journal.json>
# preview first; add --execute to admit ${walletAddress}`}</pre>
        <p className="direct-status">The browser can check your standing (above) but cannot yet build the admission transaction itself. That is a gap we intend to close, not a policy.</p>
      </>}
  </>;
}

export default function JoinPanel({
  endpoint,
  marketAddress,
  marketPhase,
  coreProgramId,
  registryProgramId,
  claimsProgramId,
  tradingProgramId,
  custodyProgramId,
  rentProgramId,
}: Readonly<{
  endpoint: string;
  marketAddress: string;
  marketPhase: string;
  coreProgramId: string;
  registryProgramId: string | null;
  claimsProgramId: string | null;
  tradingProgramId: string | null;
  custodyProgramId: string | null;
  rentProgramId: string | null;
}>) {
  const wallets = useWalletDirectoryV1();
  const [inspection, setInspection] = useState<InspectionState>({ kind: 'idle' });

  async function inspect() {
    if (wallets.address === null) return;
    if (registryProgramId === null || claimsProgramId === null || tradingProgramId === null
      || custodyProgramId === null || rentProgramId === null) {
      setInspection({ kind: 'refused', reason: 'this deployment does not name every program needed to derive your participant accounts' });
      return;
    }
    setInspection({ kind: 'working' });
    try {
      const rpc = new SolanaRpcClient(endpoint);
      const readiness = await inspectDirectParticipantReadinessV1(rpc, {
        market: marketAddress,
        owner: wallets.address,
        coreProgram: coreProgramId,
        registryProgram: registryProgramId,
        claimsProgram: claimsProgramId,
        tradingProgram: tradingProgramId,
        custodyProgram: custodyProgramId,
        rentProgram: rentProgramId,
      });
      setInspection({ kind: 'done', readiness });
    } catch (error) {
      setInspection({ kind: 'refused', reason: errorMessage(error) });
    }
  }

  return <section className="trade-v3-card" id="join">
    <header><span>05</span><div><h2>Join this market</h2><p>A participant holds a Position: per-claim balances backed by collateral in accounts derived from this Market and your wallet. Connect a wallet to see where you stand.</p></div></header>

    <WalletDirectory directory={wallets} purpose="join inspection" onConnected={() => setInspection({ kind: 'idle' })} />

    {wallets.address === null
      ? <p className="direct-status">No wallet is connected. Connecting only shares an address; nothing is signed.</p>
      : <div className="direct-actions">
        <button type="button" onClick={() => { void inspect(); }} disabled={inspection.kind === 'working'}>
          {inspection.kind === 'working' ? 'Reading your accounts…' : 'Check my standing on this market'}
        </button>
      </div>}

    {inspection.kind === 'refused' && <p className="market-refusal">Refused: {inspection.reason}</p>}

    {inspection.kind === 'done' && wallets.address !== null
      && <JoinStanding readiness={inspection.readiness} marketPhase={marketPhase} walletAddress={wallets.address} />}
  </section>;
}
