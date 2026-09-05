'use client';

import { useState } from 'react';

import WalletDirectory, { useWalletDirectoryV1 } from '@/components/WalletDirectory';
import {
  inspectDirectParticipantReadinessV1,
  type DirectParticipantReadinessV1,
} from '@dclutch/sdk/directParticipant';
import { SolanaRpcClient } from '@dclutch/sdk/rpc';
// The admission planner is typed against this app's own RPC client. The two
// classes are structurally identical and nominally distinct; importing the one
// the callee expects is smaller than widening a signature to paper over it.
import { SolanaRpcClient as WebSolanaRpcClient } from '@dclutch/sdk/rpc';
import { prepareUserPositionAdmissionV1, type PreparedAdmissionV1 } from '@/lib/userPositionAdmissionOperation';
import {
  clearFinalizedClientOperationJournalV1,
  markClientOperationSubmittedV1,
  requireSubmittedSignatureMatchV1,
  submittedClientOperationWireV1,
  transactionSignatureV1,
  writeUnsignedClientOperationJournalV1,
  type ClientOperationJournalV1,
} from '@/lib/clientOperationJournal';
import { requestWalletTransactionSignatureV1, submitSignedTransactionV1 } from '@dclutch/sdk/walletHandoff';
import { hex, sha256 } from '@dclutch/sdk/bytes';
import { type WalletDirectoryHandleV1 } from '@/components/WalletDirectory';
import { type UserPositionAdmissionRequestV1 } from '@/lib/userPositionAdmissionSnapshot';

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

type AdmissionState =
  | Readonly<{ kind: 'idle' | 'planning' }>
  | Readonly<{ kind: 'planned' | 'signing'; prepared: PreparedAdmissionV1 }>
  | Readonly<{ kind: 'submitted'; prepared: PreparedAdmissionV1; signature: string; note: string }>
  | Readonly<{ kind: 'joined'; signature: string }>
  | Readonly<{ kind: 'refused'; reason: string }>;

function browserStorage(): Storage {
  if (typeof window === 'undefined' || window.localStorage === undefined) {
    throw new Error('this browser does not expose local recovery storage, so no wallet signature was requested');
  }
  return window.localStorage;
}

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

export function JoinStanding({
  readiness,
  marketPhase,
  walletAddress,
  endpoint,
  admission,
  directory,
}: Readonly<{
  readiness: DirectParticipantReadinessV1;
  marketPhase: string;
  walletAddress: string;
  endpoint: string;
  /**
   * Everything the compiled planner's snapshot is derived from. Absent only
   * where a caller has no deployment to hand; the act is then not offered,
   * rather than offered and refused after a reader commits to it.
   */
  admission?: UserPositionAdmissionRequestV1;
  /** The connected wallet's handoff. Absent renders the plan without signing. */
  directory?: WalletDirectoryHandleV1;
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
        <AdmitInThisBrowser endpoint={endpoint} walletAddress={walletAddress} admission={admission} directory={directory} />
      </>}
  </>;
}


/**
 * Admission, composed here rather than published as a command.
 *
 * THE SENTENCE THIS REPLACES was true when it was written: "The browser cannot
 * yet build the admission transaction itself." Admission is what turns a
 * wallet into a market participant, so while that held, maker/taker trade was
 * present-but-unreachable for a stranger — you cannot trade in a market you
 * cannot join.
 *
 * The answer was not to reimplement the frame. Twenty-seven accounts with
 * per-coordinate privileges, two rent deficits and a predicted Claims receipt,
 * rewritten in TypeScript, is the mirror this application keeps convicting.
 * `plan_user_position_admission_v1` is a pure deterministic planner, so it is
 * COMPILED to wasm32 and called; every coordinate below is the Rust owner's,
 * derived from one finalized observation, and the blob is refused unless its
 * bytes match the generated artifact's digest.
 *
 * Signing and submission stay in the shell, where they belong, and stay
 * explicit: this composes and shows the exact transaction, and the reader
 * decides.
 */
/**
 * The admission act's inputs, or nothing.
 *
 * A deployment that does not name every program cannot derive the frame, and
 * the honest response is to not offer the act rather than to offer it and
 * refuse after a reader has committed to it. Undefined here is that decision,
 * made once.
 */
export function admissionRequestV1(input: Readonly<{
  market: string; owner: string; coreProgramId: string;
  registryProgramId: string | null; claimsProgramId: string | null;
  tradingProgramId: string | null; rentProgramId: string | null;
  activationCache?: string | null;
}>): UserPositionAdmissionRequestV1 | undefined {
  const { registryProgramId, claimsProgramId, tradingProgramId, rentProgramId, activationCache } = input;
  if (registryProgramId === null || claimsProgramId === null || tradingProgramId === null
    || rentProgramId === null || activationCache === null || activationCache === undefined) return undefined;
  return Object.freeze({
    market: input.market,
    owner: input.owner,
    coreProgramId: input.coreProgramId,
    claimsProgramId,
    tradingProgramId,
    registryProgramId,
    rentProgramId,
    activationCache,
  });
}

function AdmitInThisBrowser({
  endpoint,
  walletAddress,
  admission,
  directory,
}: Readonly<{
  endpoint: string;
  walletAddress: string;
  admission?: UserPositionAdmissionRequestV1;
  /** Absent in a read-only render: planning still works, signing is not offered. */
  directory?: WalletDirectoryHandleV1;
}>) {
  const [state, setState] = useState<AdmissionState>({ kind: 'idle' });
  if (admission === undefined) {
    return <p className="direct-status">This deployment does not name every program the admission frame needs, so it is not offered here. Nothing about this market has refused.</p>;
  }

  async function plan() {
    if (admission === undefined) return;
    setState({ kind: 'planning' });
    try {
      const prepared = await prepareUserPositionAdmissionV1(new WebSolanaRpcClient(endpoint), admission);
      setState({ kind: 'planned', prepared });
    } catch (error) {
      setState({ kind: 'refused', reason: error instanceof Error ? error.message : 'admission planning refused without a usable reason' });
    }
  }

  /**
   * Sign and send, under the same recovery protocol every other mutation here
   * uses: the exact unsigned intent is journaled BEFORE the wallet opens, the
   * signature is journaled before submission, the packet is sent once, and the
   * record clears only when the chain confirms it. A reload resumes that
   * signature and never sends a second one.
   */
  async function signAndSend() {
    if (state.kind !== 'planned' || directory === undefined || admission === undefined) return;
    const prepared = state.prepared;
    setState({ kind: 'signing', prepared });
    let submitted: ClientOperationJournalV1 | null = null;
    try {
      const client = new WebSolanaRpcClient(endpoint);
      const facts = await client.probe();
      const journal = await writeUnsignedClientOperationJournalV1(browserStorage(), {
        clusterGenesis: facts.genesisHash,
        market: admission.market,
        owner: walletAddress,
        operation: 'user-position-admission-v1',
        operationDigest: hex(await sha256(prepared.wireBytes)),
        intent: `admit ${walletAddress} to ${admission.market} at finalized slot ${prepared.observedSlot}`,
        plan: JSON.stringify({ position: prepared.derived.position, admission: prepared.derived.admission, generation: prepared.derived.generation }),
      });
      const signed = await requestWalletTransactionSignatureV1(client, directory.handoff(endpoint), prepared.transaction, walletAddress);
      if (!signed.complete) throw new Error('the wallet did not complete the one required signature');
      const signature = transactionSignatureV1(signed.transaction.signatures[0]!);
      submitted = await markClientOperationSubmittedV1(browserStorage(), journal, signature, signed.wireBytes);
      setState({ kind: 'submitted', prepared, signature, note: 'Saved before submission; sending the exact signed packet…' });
      const returned = await submitSignedTransactionV1(client, submittedClientOperationWireV1(submitted));
      requireSubmittedSignatureMatchV1(signature, returned);
      for (let attempt = 0; attempt < 30; attempt += 1) {
        const status = (await client.signatureStatuses([signature]))[0];
        if (status?.known && status.succeeded === false) {
          setState({ kind: 'submitted', prepared, signature, note: `The chain reports an error (${status.errorText ?? 'unnamed chain error'}). This submitted record stays saved because it cannot be safely replayed or discarded.` });
          return;
        }
        if (status?.known && status.succeeded === true) {
          await clearFinalizedClientOperationJournalV1(browserStorage(), submitted);
          setState({ kind: 'joined', signature });
          return;
        }
        await new Promise<void>((resolve) => setTimeout(resolve, 1_000));
      }
      setState({ kind: 'submitted', prepared, signature, note: 'Not finalized yet. You can close this page; reloading resumes this exact signature and never submits it again.' });
    } catch (error) {
      const reason = error instanceof Error ? error.message : 'admission refused without a usable reason';
      if (submitted !== null) setState({ kind: 'submitted', prepared, signature: submitted.signature!, note: `${reason} The submitted record stays saved; reloading never resubmits it.` });
      else setState({ kind: 'refused', reason });
    }
  }


  return <>
    <p className="direct-status">Joining is composed in this browser by the <strong>compiled Rust planner</strong> — the same
    `plan_user_position_admission_v1` the operator toolchain runs, built to WebAssembly and checked against
    its recorded digest before it executes. It reads one finalized observation, derives every one of the
    twenty-five accounts it authenticates, and returns the exact unsigned transaction. Nothing is signed
    until you say so.</p>
    <div className="direct-actions">
      <button type="button" disabled={state.kind === 'planning'} onClick={() => void plan()}>
        {state.kind === 'planning' ? 'Reading finalized state…' : 'Join this market'}
      </button>
    </div>
    {state.kind === 'refused' && <p className="market-refusal">Refused: {state.reason}</p>}
    {(state.kind === 'planned' || state.kind === 'signing' || state.kind === 'submitted') && <dl className="detail-facts">
      <div><dt>Your Position</dt><dd><code>{state.prepared.derived.position}</code></dd></div>
      <div><dt>Your admission record</dt><dd><code>{state.prepared.derived.admission}</code></dd></div>
      <div><dt>Refundable storage deposit</dt><dd>{state.prepared.plan.positionTopUpLamports} + {state.prepared.plan.admissionTopUpLamports} lamports</dd></div>
      <div><dt>Transaction</dt><dd>{state.prepared.wireBytes.length} bytes · one signer · finalized slot {state.prepared.observedSlot}</dd></div>
      <div><dt>Signer</dt><dd><code>{walletAddress}</code></dd></div>
    </dl>}
    {(state.kind === 'planned' || state.kind === 'signing') && directory !== undefined && <div className="direct-actions">
      <button type="button" disabled={state.kind === 'signing'} onClick={() => void signAndSend()}>
        {state.kind === 'signing' ? 'Waiting for your wallet…' : 'Sign and join'}
      </button>
    </div>}
    {state.kind === 'submitted' && <p className="direct-status" aria-live="polite">Submitted as <code>{state.signature}</code>. {state.note}</p>}
    {state.kind === 'joined' && <p className="direct-status" aria-live="polite">You are a participant in this market. Signature <code>{state.signature}</code> is confirmed; your Position and admission record exist on chain.</p>}
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
  activationCache,
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
  /** This deployment's Registry activation cache. Absent hides the act. */
  activationCache?: string | null;
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
    <header><span>05</span><div><h2>Join this market</h2><p>Connect a wallet to see where you stand.</p></div></header>

    <WalletDirectory directory={wallets} onConnected={() => setInspection({ kind: 'idle' })} />

    {wallets.address === null
      ? <p className="direct-status">No wallet connected.</p>
      : <div className="direct-actions">
        <button type="button" onClick={() => { void inspect(); }} disabled={inspection.kind === 'working'}>
          {inspection.kind === 'working' ? 'Reading your accounts…' : 'Check my standing on this market'}
        </button>
      </div>}

    <div aria-live="polite">
      {inspection.kind === 'refused' && <p className="market-refusal">Refused: {inspection.reason}</p>}

      {inspection.kind === 'done' && wallets.address !== null
        && <JoinStanding
          readiness={inspection.readiness}
          marketPhase={marketPhase}
          walletAddress={wallets.address}
          endpoint={endpoint}
          admission={admissionRequestV1({
            market: marketAddress, owner: wallets.address, coreProgramId,
            registryProgramId, claimsProgramId, tradingProgramId, rentProgramId, activationCache,
          })}
        />}
    </div>
  </section>;
}
