'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';

import ConsoleHeader from '@/components/ConsoleHeader';
import WalletDirectory, { useWalletDirectoryV1 } from '@/components/WalletDirectory';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  clearFinalizedClientOperationJournalV1,
  discardUnsignedClientOperationJournalV1,
  findClientOperationJournalV1,
  markClientOperationSubmittedV1,
  requireSubmittedSignatureMatchV1,
  submittedClientOperationWireV1,
  transactionSignatureV1,
  writeUnsignedClientOperationJournalV1,
  type ClientOperationJournalV1,
} from '@/lib/clientOperationJournal';
import { useDeploymentFieldV1, useDeploymentV1 } from '@/lib/deploymentStore';
import { SolanaRpcClient } from '@/lib/rpc';
import {
  acquireSourceReadinessV1,
  buildSourceReadinessTransactionV1,
  loadSourceReadinessWasmV1,
  type SourceReadinessAcquisitionV1,
  type SourceReadinessTransactionV1,
  type SourceReadinessWasmV1,
} from '@/lib/sourceReadinessV1';
import {
  restoreSourceReadinessJournalV1,
  sourceReadinessJournalInputV1,
  sourceReadinessPoststateCompletesV1,
} from '@/lib/sourceReadinessOperationV1';
import {
  acquireSourceTerminalV1,
  buildSourceTerminalTransactionV1,
  type SourceTerminalAcquisitionV1,
  type SourceTerminalTransactionV1,
} from '@/lib/sourceTerminalV1';
import {
  restoreSourceTerminalJournalV1,
  sourceTerminalJournalInputV1,
} from '@/lib/sourceTerminalOperationV1';
import {
  acquireSourceCloseFundV1,
  buildSourceCloseFundTransactionV1,
  verifySourceCloseFundFinalizedV1,
  type SourceCloseFundAcquisitionV1,
  type SourceCloseFundTransactionV1,
} from '@/lib/sourceCloseFundV1';
import {
  restoreSourceCloseFundJournalV1,
  sourceCloseFundJournalInputV1,
} from '@/lib/sourceCloseFundOperationV1';
import {
  acquireSourceProviderReclaimV1,
  acquireSourceProviderSubmitV1,
  loadSourceProviderWasmV1,
  sourceProviderReclaimPoststateCompletesV1,
  sourceProviderSubmitPoststateCompletesV1,
  type SourceProviderReclaimAcquisitionV1,
  type SourceProviderSubmitAcquisitionV1,
} from '@/lib/sourceProviderV1';
import { sourceProviderJournalInputV1, restoreSourceProviderJournalV1 } from '@/lib/sourceProviderOperationV1';
import { sourceProviderSubmitJournalInputV1, restoreSourceProviderSubmitJournalV1 } from '@/lib/sourceProviderSubmitOperationV1';
import {
  requestWalletCosignTransactionV1,
  requestWalletSubmitCosignTransactionV1,
  requestWalletTransactionSignatureV1,
  submitSignedTransactionV1,
} from '@/lib/walletHandoff';

type ResolutionStateV1 =
  | Readonly<{ kind: 'idle' | 'reading'; message: string }>
  | Readonly<{ kind: 'observed'; acquisition: SourceReadinessAcquisitionV1; message: string }>
  | Readonly<{ kind: 'prepared' | 'signing'; acquisition: SourceReadinessAcquisitionV1; transaction: SourceReadinessTransactionV1; journal: ClientOperationJournalV1; message: string }>
  | Readonly<{ kind: 'submitted'; acquisition: SourceReadinessAcquisitionV1 | null; journal: ClientOperationJournalV1; message: string }>
  | Readonly<{ kind: 'complete'; acquisition: SourceReadinessAcquisitionV1; signature: string | null; message: string }>
  | Readonly<{ kind: 'refused'; message: string; journal: ClientOperationJournalV1 | null }>;

type ProviderReclaimStateV1 =
  | Readonly<{ kind: 'idle' | 'reading' | 'complete'; message: string }>
  | Readonly<{ kind: 'prepared' | 'signing'; acquisition: SourceProviderReclaimAcquisitionV1; journal: ClientOperationJournalV1; message: string }>
  | Readonly<{ kind: 'submitted'; acquisition: SourceProviderReclaimAcquisitionV1 | null; journal: ClientOperationJournalV1; message: string }>
  | Readonly<{ kind: 'refused'; journal: ClientOperationJournalV1 | null; message: string }>;

type ProviderSubmitStateV1 =
  | Readonly<{ kind: 'idle' | 'reading' | 'complete'; message: string }>
  | Readonly<{ kind: 'prepared' | 'signing'; acquisition: SourceProviderSubmitAcquisitionV1; journal: ClientOperationJournalV1; message: string }>
  | Readonly<{ kind: 'submitted'; acquisition: SourceProviderSubmitAcquisitionV1 | null; journal: ClientOperationJournalV1; message: string }>
  | Readonly<{ kind: 'refused'; journal: ClientOperationJournalV1 | null; message: string }>;

type SourceTerminalStateV1 =
  | Readonly<{ kind: 'idle' | 'reading' | 'complete'; message: string; acquisition?: SourceTerminalAcquisitionV1 }>
  | Readonly<{ kind: 'prepared' | 'signing'; message: string; acquisition: SourceTerminalAcquisitionV1; transaction: SourceTerminalTransactionV1; journal: ClientOperationJournalV1 }>
  | Readonly<{ kind: 'submitted'; message: string; acquisition: SourceTerminalAcquisitionV1 | null; journal: ClientOperationJournalV1 }>
  | Readonly<{ kind: 'refused'; message: string; journal: ClientOperationJournalV1 | null }>;

type SourceCloseFundStateV1 =
  | Readonly<{ kind: 'idle' | 'reading' | 'complete'; message: string; acquisition?: SourceCloseFundAcquisitionV1 }>
  | Readonly<{ kind: 'prepared' | 'signing'; message: string; acquisition: SourceCloseFundAcquisitionV1; transaction: SourceCloseFundTransactionV1; journal: ClientOperationJournalV1 }>
  | Readonly<{ kind: 'submitted'; message: string; acquisition: SourceCloseFundAcquisitionV1 | null; journal: ClientOperationJournalV1 }>
  | Readonly<{ kind: 'refused'; message: string; journal: ClientOperationJournalV1 | null }>;

const pause = () => new Promise<void>((resolve) => setTimeout(resolve, 1_000));
const errorMessage = (error: unknown) => error instanceof Error ? error.message : 'the Source readiness act refused without a usable reason';
const short = (value: string) => `${value.slice(0, 7)}…${value.slice(-6)}`;

function browserStorage(): Storage {
  if (typeof window === 'undefined' || window.localStorage === undefined) {
    throw new Error('this browser exposes no durable recovery storage, so no wallet signature was requested');
  }
  return window.localStorage;
}

function routeCopy(route: SourceReadinessAcquisitionV1['plan']['route']): Readonly<{
  title: string;
  outcome: string;
  authority: string;
  button: string | null;
}> {
  if (route === 'create') return Object.freeze({
    title: 'Create the Source funding state',
    outcome: 'Bind the Market’s unique three-entry funding subset and create its pending Source state.',
    authority: 'The Rust planner selects the records and entries. Your wallet is only the transaction payer.',
    button: 'Prepare CreateFund',
  });
  if (route === 'activate') return Object.freeze({
    title: 'Activate the pending funds',
    outcome: 'Credit the immutable beneficiary and publish the Market-bound funding activation receipt.',
    authority: 'Permissionless Resolution instruction; your wallet pays the fee and exact receipt top-up.',
    button: 'Prepare ActivateFund',
  });
  if (route === 'accept') return Object.freeze({
    title: 'Accept funding as ready',
    outcome: 'Have Core reauthenticate the active Source ledger and mark this Market ready for resolution.',
    authority: 'Permissionless Core instruction; your wallet pays only the transaction fee.',
    button: 'Prepare VerifyFundReady',
  });
  if (route === 'complete') return Object.freeze({
    title: 'Resolution funding is ready',
    outcome: 'The selected Market already passes the current CreateFund and VerifyFundReady chain checks.',
    authority: 'No wallet act remains. The page reauthenticated the current releases, records, ledger, and receipt.',
    button: null,
  });
  return Object.freeze({
    title: 'Founding already consumed readiness',
    outcome: 'Atomic founding created the live Source state; there is no separate readiness transaction to send.',
    authority: 'No wallet act remains.',
    button: null,
  });
}

function SourceTerminalPanel({
  client, directory, endpoint, market, programs, wasmPromise,
}: Readonly<{
  client: SolanaRpcClient;
  directory: ReturnType<typeof useWalletDirectoryV1>;
  endpoint: string;
  market: string;
  programs: Readonly<{ coreProgram: string; registryProgram: string; resolutionProgram: string }>;
  wasmPromise: Promise<SourceReadinessWasmV1>;
}>) {
  const wallet = directory.address;
  const [state, setState] = useState<SourceTerminalStateV1>({ kind: 'idle', message: 'Read the terminal Source and Product graph to determine whether admission remains.' });
  const acquire = useCallback(async () => acquireSourceTerminalV1(client, await wasmPromise, market.trim(), programs),
    [client, market, programs, wasmPromise]);

  const poll = useCallback(async (journal: ClientOperationJournalV1, before: SourceTerminalAcquisitionV1 | null, alive: () => boolean = () => true) => {
    if (journal.phase !== 'submitted' || journal.signature === null) throw new Error('submitted terminal admission has no exact signature');
    await restoreSourceTerminalJournalV1(journal);
    for (let attempt = 0; attempt < 30 && alive(); attempt += 1) {
      try {
        const [status, after] = await Promise.all([
          client.signatureStatuses([journal.signature]).then((values) => values[0]), acquire(),
        ]);
        if (!alive()) return;
        if (status?.known && status.succeeded === false) {
          setState({ kind: 'submitted', acquisition: before, journal, message: `The chain reports an error (${status.errorText ?? 'unnamed chain error'}). The exact signed record remains saved.` });
          return;
        }
        if (status?.known && status.confirmationStatus === 'finalized' && after.plan.route === 'complete') {
          await clearFinalizedClientOperationJournalV1(browserStorage(), journal);
          setState({ kind: 'complete', acquisition: after, message: 'Core has admitted the exact terminal certificate. Recovery is cleared.' });
          return;
        }
        setState({ kind: 'submitted', acquisition: before, journal, message: 'Waiting for the exact signature and Rust-authenticated Terminal poststate. Reloading resumes; it never resubmits.' });
      } catch (error) {
        if (alive()) setState({ kind: 'submitted', acquisition: before, journal, message: `${errorMessage(error)} The exact submitted record remains saved.` });
        return;
      }
      await pause();
    }
  }, [acquire, client]);

  useEffect(() => {
    if (wallet === null || market.trim() === '') return;
    let alive = true;
    void (async () => {
      try {
        const admission = await client.assertMutationCluster();
        const journal = await findClientOperationJournalV1(browserStorage(), {
          clusterGenesis: admission.genesisHash, market: market.trim(), owner: wallet,
        }, 'source-terminal-v1');
        if (!alive || journal === null) return;
        if (journal.phase === 'submitted') {
          setState({ kind: 'submitted', acquisition: null, journal, message: 'Resuming the saved terminal signature and poststate; nothing is resubmitted.' });
          void poll(journal, null, () => alive);
        } else {
          await restoreSourceTerminalJournalV1(journal);
          setState({ kind: 'refused', journal, message: 'An unsigned terminal packet remains saved. Discard it explicitly, then read finalized state again.' });
        }
      } catch (error) {
        if (alive) setState({ kind: 'refused', journal: null, message: `Recovery refused: ${errorMessage(error)}` });
      }
    })();
    return () => { alive = false; };
  }, [client, market, poll, wallet]);

  async function read() {
    setState({ kind: 'reading', message: 'Reacquiring the current Source, Product graph, certificate, releases, and funding ledger…' });
    try {
      const acquisition = await acquire();
      setState(acquisition.plan.route === 'complete'
        ? { kind: 'complete', acquisition, message: 'The Market already names this exact terminal certificate.' }
        : { kind: 'idle', acquisition, message: 'Terminal certificate authenticated. One permissionless Core admission remains.' });
    } catch (error) { setState({ kind: 'refused', journal: null, message: errorMessage(error) }); }
  }

  async function prepare() {
    if (wallet === null) return;
    setState({ kind: 'reading', message: 'Rechecking exact terminal state, cluster identity, and current blockhash…' });
    try {
      const acquisition = await acquire();
      if (acquisition.plan.route !== 'admit') throw new Error('terminal admission is already complete; no wallet act remains');
      const admission = await client.assertMutationCluster();
      const transaction = buildSourceTerminalTransactionV1(acquisition, wallet,
        await client.latestMutationBlockhash(acquisition.plan.observedSlot));
      const journal = await writeUnsignedClientOperationJournalV1(browserStorage(),
        await sourceTerminalJournalInputV1({ clusterGenesis: admission.genesisHash, market: market.trim(), owner: wallet }, acquisition, transaction));
      setState({ kind: 'prepared', acquisition, transaction, journal, message: 'Saved the exact unsigned terminal admission. The next button requests one wallet signature.' });
    } catch (error) { setState({ kind: 'refused', journal: null, message: errorMessage(error) }); }
  }

  async function signAndSubmit() {
    if (state.kind !== 'prepared' || wallet === null) return;
    const prepared = state;
    setState({ ...prepared, kind: 'signing', message: 'Waiting for the payer wallet…' });
    let submitted: ClientOperationJournalV1 | null = null;
    try {
      const signed = await requestWalletTransactionSignatureV1(client, directory.handoff(endpoint), prepared.transaction.transaction, wallet);
      if (!signed.complete) throw new Error('wallet did not complete the sole payer signature');
      const signature = transactionSignatureV1(signed.transaction.signatures[0]!);
      submitted = await markClientOperationSubmittedV1(browserStorage(), prepared.journal, signature, signed.wireBytes);
      setState({ kind: 'submitted', acquisition: prepared.acquisition, journal: submitted, message: 'Signed packet saved; submitting once with preflight…' });
      requireSubmittedSignatureMatchV1(signature, await submitSignedTransactionV1(client, submittedClientOperationWireV1(submitted)));
      await poll(submitted, prepared.acquisition);
    } catch (error) {
      setState(submitted === null
        ? { kind: 'refused', journal: prepared.journal, message: errorMessage(error) }
        : { kind: 'submitted', acquisition: prepared.acquisition, journal: submitted, message: `${errorMessage(error)} The signed record remains saved and is never automatically resubmitted.` });
    }
  }

  async function discard(journal: ClientOperationJournalV1) {
    try {
      await discardUnsignedClientOperationJournalV1(browserStorage(), journal);
      setState({ kind: 'idle', message: 'Unsigned terminal admission discarded. Read finalized state again.' });
    } catch (error) { setState({ kind: 'refused', journal, message: errorMessage(error) }); }
  }

  const acquisition = 'acquisition' in state ? state.acquisition ?? null : null;
  return <section className="workbench-actions">
    <header><span>Terminal admission</span><h2>Commit the terminal result to Core</h2><p>Authenticate the terminal Source decision and Product-wide selector, then have Core name the exact Resolution certificate.</p></header>
    {acquisition !== null && <Card className="ready"><CardHeader><span className="operator-status ready-to-preflight">{acquisition.plan.route}</span><CardTitle>{acquisition.plan.route === 'complete' ? 'Terminal already admitted' : 'Admission ready'}</CardTitle><CardDescription>The Rust owner derives the Product graph, certificate, caller authority, funding entries, and all 22 protocol accounts.</CardDescription></CardHeader><CardContent><dl className="operator-action-contract"><div><dt>Finalized slot</dt><dd>{acquisition.plan.observedSlot}</dd></div><div><dt>Selector</dt><dd>{acquisition.plan.facts.selector}</dd></div><div><dt>Outcomes</dt><dd>{acquisition.plan.facts.outcomeCount}</dd></div><div><dt>Wallet</dt><dd>fee payer only</dd></div></dl></CardContent></Card>}
    <Alert variant={state.kind === 'refused' ? 'destructive' : 'default'} aria-live="polite"><AlertTitle>{state.kind === 'refused' ? 'Refused safely' : 'Terminal status'}</AlertTitle><AlertDescription>{state.message}</AlertDescription></Alert>
    {(state.kind === 'idle' || state.kind === 'refused') && acquisition === null && <Button type="button" disabled={market.trim() === ''} onClick={() => void read()}>Read terminal admission</Button>}
    {state.kind === 'idle' && acquisition?.plan.route === 'admit' && <Button type="button" disabled={wallet === null} onClick={() => void prepare()}>{wallet === null ? 'Connect the payer wallet above' : 'Prepare AdmitTerminal'}</Button>}
    {state.kind === 'prepared' && <Button type="button" onClick={() => void signAndSubmit()}>Sign and submit admission</Button>}
    {state.kind === 'signing' && <Button type="button" disabled>Waiting for wallet…</Button>}
    {state.kind === 'refused' && state.journal?.phase === 'unsigned' && <Button type="button" variant="outline" onClick={() => void discard(state.journal!)}>Discard unsigned admission</Button>}
    <footer><strong>Safety contract</strong><span>One finalized 21-account observation selects one signer-free 22-account protocol instruction. The browser saves before signing and submission, sends once, and clears only when Rust reauthenticates the exact Terminal receipt.</span></footer>
  </section>;
}

function SourceCloseFundPanel({
  client, directory, endpoint, market, programs, wasmPromise,
}: Readonly<{
  client: SolanaRpcClient;
  directory: ReturnType<typeof useWalletDirectoryV1>;
  endpoint: string;
  market: string;
  programs: Readonly<{ coreProgram: string; registryProgram: string; resolutionProgram: string }>;
  wasmPromise: Promise<SourceReadinessWasmV1>;
}>) {
  const wallet = directory.address;
  const [state, setState] = useState<SourceCloseFundStateV1>({ kind: 'idle', message: 'Read the Retiring Source to select exact receipt prepayment or direct close.' });
  const acquire = useCallback(async () => acquireSourceCloseFundV1(client, await wasmPromise, market.trim(), programs),
    [client, market, programs, wasmPromise]);

  const poll = useCallback(async (journal: ClientOperationJournalV1, before: SourceCloseFundAcquisitionV1 | null, alive: () => boolean = () => true) => {
    if (journal.phase !== 'submitted' || journal.signature === null) throw new Error('submitted Source close act has no exact signature');
    const restored = await restoreSourceCloseFundJournalV1(journal);
    const wasm = await wasmPromise;
    const restoredAcquisition: SourceCloseFundAcquisitionV1 = Object.freeze({ plan: restored.plan,
      planJson: (JSON.parse(journal.plan) as { rustPlan: string }).rustPlan, snapshotJson: '{}', observationAddresses: [] });
    for (let attempt = 0; attempt < 30 && alive(); attempt += 1) {
      try {
        const status = (await client.signatureStatuses([journal.signature]))[0];
        if (!alive()) return;
        if (status?.known && status.succeeded === false) {
          setState({ kind: 'submitted', acquisition: before, journal, message: `The chain refused this exact ${restored.intent.route} (${status.errorText ?? 'unnamed chain error'}). Its signed record remains saved.` });
          return;
        }
        if (status?.known && status.confirmationStatus === 'finalized') {
          if (restored.intent.route === 'prepay') {
            const after = await acquire();
            if (after.plan.route !== 'close') throw new Error('finalized receipt prepay did not select the exact direct close');
            await clearFinalizedClientOperationJournalV1(browserStorage(), journal);
            setState({ kind: 'idle', acquisition: after, message: 'Receipt prepayment is finalized and journaled. The fresh Rust plan now exposes direct CloseFund.' });
            return;
          }
          const completion = await verifySourceCloseFundFinalizedV1(client, wasm, restoredAcquisition, programs.resolutionProgram);
          await clearFinalizedClientOperationJournalV1(browserStorage(), journal);
          setState({ kind: 'complete', message: `Source and funding ledger closed; receipt ${short(completion.receipt)} is finalized and byte-exact.` });
          return;
        }
        setState({ kind: 'submitted', acquisition: before, journal, message: `Waiting for the exact ${restored.intent.route} signature and authenticated finalized poststate. Reloading never resubmits.` });
      } catch (error) {
        if (alive()) setState({ kind: 'submitted', acquisition: before, journal, message: `${errorMessage(error)} The signed record remains saved.` });
        return;
      }
      await pause();
    }
  }, [acquire, client, programs.resolutionProgram, wasmPromise]);

  useEffect(() => {
    if (wallet === null || market.trim() === '') return;
    let alive = true;
    void (async () => {
      try {
        const admission = await client.assertMutationCluster();
        const journal = await findClientOperationJournalV1(browserStorage(), {
          clusterGenesis: admission.genesisHash, market: market.trim(), owner: wallet,
        }, 'source-close-fund-v1');
        if (!alive || journal === null) return;
        if (journal.phase === 'submitted') {
          setState({ kind: 'submitted', acquisition: null, journal, message: 'Resuming the saved Source close signature and poststate; nothing is resubmitted.' });
          void poll(journal, null, () => alive);
        } else {
          await restoreSourceCloseFundJournalV1(journal);
          setState({ kind: 'refused', journal, message: 'An unsigned Source close packet remains saved. Discard it explicitly before replanning.' });
        }
      } catch (error) {
        if (alive) setState({ kind: 'refused', journal: null, message: `Recovery refused: ${errorMessage(error)}` });
      }
    })();
    return () => { alive = false; };
  }, [client, market, poll, wallet]);

  async function read() {
    setState({ kind: 'reading', message: 'Reacquiring the exact Retiring Source, releases, funding ledger, certificate, receipt, and rent…' });
    try {
      const acquisition = await acquire();
      setState({ kind: 'idle', acquisition, message: acquisition.plan.route === 'prepay'
        ? `The canonical closure receipt needs exactly ${acquisition.plan.prepay!.lamports} lamports before CloseFund.`
        : `Receipt is exactly prepaid. V7 direct CloseFund will return ${acquisition.plan.facts.refundLamports} lamports to the immutable beneficiary.` });
    } catch (error) { setState({ kind: 'refused', journal: null, message: errorMessage(error) }); }
  }

  async function prepare() {
    if (wallet === null) return;
    setState({ kind: 'reading', message: 'Rechecking finalized state, cluster identity, and blockhash before saving…' });
    try {
      const acquisition = await acquire();
      const admission = await client.assertMutationCluster();
      const transaction = buildSourceCloseFundTransactionV1(acquisition, wallet,
        await client.latestMutationBlockhash(acquisition.plan.observedSlot));
      const journal = await writeUnsignedClientOperationJournalV1(browserStorage(),
        await sourceCloseFundJournalInputV1({ clusterGenesis: admission.genesisHash, market: market.trim(), owner: wallet }, acquisition, transaction));
      setState({ kind: 'prepared', acquisition, transaction, journal, message: `Saved the exact unsigned ${acquisition.plan.route}. The next button requests one payer signature.` });
    } catch (error) { setState({ kind: 'refused', journal: null, message: errorMessage(error) }); }
  }

  async function signAndSubmit() {
    if (state.kind !== 'prepared' || wallet === null) return;
    const prepared = state;
    setState({ ...prepared, kind: 'signing', message: 'Waiting for the payer wallet…' });
    let submitted: ClientOperationJournalV1 | null = null;
    try {
      const signed = await requestWalletTransactionSignatureV1(client, directory.handoff(endpoint), prepared.transaction.transaction, wallet);
      if (!signed.complete) throw new Error('wallet did not complete the sole payer signature');
      const signature = transactionSignatureV1(signed.transaction.signatures[0]!);
      submitted = await markClientOperationSubmittedV1(browserStorage(), prepared.journal, signature, signed.wireBytes);
      setState({ kind: 'submitted', acquisition: prepared.acquisition, journal: submitted, message: 'Signed packet saved; submitting once with preflight…' });
      requireSubmittedSignatureMatchV1(signature, await submitSignedTransactionV1(client, submittedClientOperationWireV1(submitted)));
      await poll(submitted, prepared.acquisition);
    } catch (error) {
      setState(submitted === null
        ? { kind: 'refused', journal: prepared.journal, message: errorMessage(error) }
        : { kind: 'submitted', acquisition: prepared.acquisition, journal: submitted, message: `${errorMessage(error)} The signed record remains saved and is never automatically resubmitted.` });
    }
  }

  async function discard(journal: ClientOperationJournalV1) {
    try {
      await discardUnsignedClientOperationJournalV1(browserStorage(), journal);
      setState({ kind: 'idle', message: 'Unsigned Source close act discarded. Read finalized state again.' });
    } catch (error) { setState({ kind: 'refused', journal, message: errorMessage(error) }); }
  }

  const acquisition = 'acquisition' in state ? state.acquisition ?? null : null;
  return <section className="workbench-actions">
    <header><span>Funding close</span><h2>Discharge terminal Source funds</h2><p>Prepay the durable receipt if needed, then close Source and its three-entry ledger through the direct Resolution route.</p></header>
    {acquisition !== null && <Card className="ready"><CardHeader><span className="operator-status ready-to-preflight">{acquisition.plan.route}</span><CardTitle>{acquisition.plan.route === 'prepay' ? 'Prepay the closure receipt' : 'Direct close ready'}</CardTitle><CardDescription>The Market and terminal Source select the certificate, closure sequence, beneficiary, refund, and exact 19/21-account frame.</CardDescription></CardHeader><CardContent><dl className="operator-action-contract"><div><dt>Finalized slot</dt><dd>{acquisition.plan.observedSlot}</dd></div><div><dt>Receipt prepay</dt><dd>{acquisition.plan.prepay?.lamports ?? 'complete'} lamports</dd></div><div><dt>Refund</dt><dd>{acquisition.plan.facts.refundLamports ?? 'after prepay'} lamports</dd></div><div><dt>Wallet</dt><dd>payer only</dd></div></dl></CardContent></Card>}
    <Alert variant={state.kind === 'refused' ? 'destructive' : 'default'} aria-live="polite"><AlertTitle>{state.kind === 'refused' ? 'Refused safely' : 'Close status'}</AlertTitle><AlertDescription>{state.message}</AlertDescription></Alert>
    {(state.kind === 'idle' || state.kind === 'refused') && acquisition === null && <Button type="button" disabled={market.trim() === ''} onClick={() => void read()}>Read Source close</Button>}
    {state.kind === 'idle' && acquisition !== null && <Button type="button" disabled={wallet === null} onClick={() => void prepare()}>{wallet === null ? 'Connect the payer wallet above' : `Prepare ${acquisition.plan.route === 'prepay' ? 'receipt prepay' : 'CloseFund'}`}</Button>}
    {state.kind === 'prepared' && <Button type="button" onClick={() => void signAndSubmit()}>Sign and submit {state.transaction.route}</Button>}
    {state.kind === 'signing' && <Button type="button" disabled>Waiting for wallet…</Button>}
    {state.kind === 'refused' && state.journal?.phase === 'unsigned' && <Button type="button" variant="outline" onClick={() => void discard(state.journal!)}>Discard unsigned close act</Button>}
    <footer><strong>Safety contract</strong><span>Prepay and close are separate durable acts. Each is replanned from finalized state, saved before signing and submission, sent once, and cleared only after the next route or exact typed closure receipt authenticates.</span></footer>
  </section>;
}

function ProviderSubmitPanel({
  client,
  directory,
  endpoint,
  market,
  programs,
}: Readonly<{
  client: SolanaRpcClient;
  directory: ReturnType<typeof useWalletDirectoryV1>;
  endpoint: string;
  market: string;
  programs: Readonly<{ coreProgram: string; registryProgram: string; resolutionProgram: string }>;
}>) {
  const [encodedVaa, setEncodedVaa] = useState('');
  const [postBody, setPostBody] = useState('');
  const [lookupTable, setLookupTable] = useState('');
  const [reclaimAfter, setReclaimAfter] = useState('');
  const [state, setState] = useState<ProviderSubmitStateV1>({
    kind: 'idle',
    message: 'Supply one verified EncodedVaa, its exact Receiver body, one frozen provider table, and the reclaim time.',
  });
  const wallet = directory.address;
  const wasmPromise = useMemo(() => loadSourceProviderWasmV1(), []);

  const poll = useCallback(async (
    journal: ClientOperationJournalV1,
    acquisition: SourceProviderSubmitAcquisitionV1 | null,
    alive: () => boolean = () => true,
  ) => {
    if (journal.phase !== 'submitted' || journal.signature === null) throw new Error('submitted provider creation has no exact signature');
    const wasm = await wasmPromise;
    const restored = await restoreSourceProviderSubmitJournalV1(journal);
    for (let attempt = 0; attempt < 30 && alive(); attempt += 1) {
      try {
        const status = (await client.signatureStatuses([journal.signature]))[0];
        if (!alive()) return;
        if (status?.known && status.succeeded === false) {
          setState({ kind: 'submitted', acquisition, journal, message: `The chain refused this exact provider submit (${status.errorText ?? 'unnamed chain error'}). Its signed record remains saved.` });
          return;
        }
        if (status?.known && status.confirmationStatus === 'finalized' && status.slot !== null
            && await sourceProviderSubmitPoststateCompletesV1(client, wasm, restored.rustPlan, status.slot)) {
          await clearFinalizedClientOperationJournalV1(browserStorage(), journal);
          setState({ kind: 'complete', message: 'Finalized: the Receiver update exists and the Resolution lifecycle reauthenticates its exact request, signer, release, rent, and update digest.' });
          return;
        }
        setState({ kind: 'submitted', acquisition, journal, message: 'Waiting for the exact signature and Rust-authenticated lifecycle/update poststate. Nothing is resubmitted.' });
      } catch (error) {
        if (alive()) setState({ kind: 'submitted', acquisition, journal, message: `${errorMessage(error)} The signed packet remains saved; reload only resumes its poststate check.` });
        return;
      }
      await pause();
    }
  }, [client, wasmPromise]);

  useEffect(() => {
    if (wallet === null || market.trim() === '') return;
    let current = true;
    void (async () => {
      try {
        const admission = await client.assertMutationCluster();
        const journal = await findClientOperationJournalV1(browserStorage(), {
          clusterGenesis: admission.genesisHash,
          market: market.trim(),
          owner: wallet,
        }, 'source-provider-submit-v1');
        if (!current || journal === null) return;
        if (journal.phase === 'submitted') {
          setState({ kind: 'submitted', acquisition: null, journal, message: 'Resuming the saved provider-submit signature and exact finalized poststate check. Nothing is resubmitted.' });
          void poll(journal, null, () => current);
        } else {
          await restoreSourceProviderSubmitJournalV1(journal);
          setState({ kind: 'refused', journal, message: 'An unsigned provider submit is saved for this chain, Market, and wallet. Its fresh update key was intentionally not persisted; discard it and prepare again.' });
        }
      } catch (error) {
        if (current) setState({ kind: 'refused', journal: null, message: `Provider-submit recovery refused: ${errorMessage(error)}` });
      }
    })();
    return () => { current = false; };
  }, [client, market, poll, wallet]);

  async function prepare() {
    if (wallet === null || market.trim() === '' || encodedVaa.trim() === '' || postBody.trim() === ''
        || lookupTable.trim() === '' || reclaimAfter.trim() === '') return;
    setState({ kind: 'reading', message: 'Walking the Market’s current Source and Pyth release graph, then rejoining the exact table-backed account frame…' });
    try {
      const admission = await client.assertMutationCluster();
      const acquisition = await acquireSourceProviderSubmitV1(client, await wasmPromise, {
        market: market.trim(),
        payer: wallet,
        encodedVaa: encodedVaa.trim(),
        postUpdateBodyBase64: postBody.trim(),
        lookupTable: lookupTable.trim(),
        reclaimAfterUnixSeconds: reclaimAfter.trim(),
      }, programs);
      const input = await sourceProviderSubmitJournalInputV1({
        clusterGenesis: admission.genesisHash,
        market: acquisition.market,
        owner: wallet,
      }, acquisition);
      const journal = await writeUnsignedClientOperationJournalV1(browserStorage(), input);
      setState({ kind: 'prepared', acquisition, journal, message: 'Saved the exact unsigned Rust message. The next act signs with the fresh in-memory Receiver update, then asks this wallet only for the submitter/fee-payer signature.' });
    } catch (error) {
      setState({ kind: 'refused', journal: null, message: errorMessage(error) });
    }
  }

  async function signAndSubmit() {
    if (state.kind !== 'prepared' || wallet === null) return;
    const prepared = state;
    setState({ ...prepared, kind: 'signing', message: 'Signing with the fresh update account, then waiting for the submitter wallet…' });
    let submitted: ClientOperationJournalV1 | null = null;
    try {
      prepared.acquisition.transaction.sign([prepared.acquisition.update]);
      const signed = await requestWalletSubmitCosignTransactionV1(
        client,
        directory.handoff(endpoint),
        prepared.acquisition.transaction,
        wallet,
        prepared.acquisition.update.publicKey.toBase58(),
      );
      const signature = transactionSignatureV1(signed.transaction.signatures[0]!);
      submitted = await markClientOperationSubmittedV1(browserStorage(), prepared.journal, signature, signed.wireBytes);
      setState({ kind: 'submitted', acquisition: prepared.acquisition, journal: submitted, message: 'Saved the fully signed packet before transport; sending these exact bytes once.' });
      const returned = await submitSignedTransactionV1(client, submittedClientOperationWireV1(submitted));
      requireSubmittedSignatureMatchV1(signature, returned);
      await poll(submitted, prepared.acquisition);
    } catch (error) {
      setState(submitted === null
        ? { kind: 'refused', journal: prepared.journal, message: errorMessage(error) }
        : { kind: 'submitted', acquisition: prepared.acquisition, journal: submitted, message: `${errorMessage(error)} The exact signed packet remains saved and is never resubmitted automatically.` });
    }
  }

  async function discard() {
    if (state.kind !== 'refused' || state.journal?.phase !== 'unsigned') return;
    try {
      await discardUnsignedClientOperationJournalV1(browserStorage(), state.journal);
      setState({ kind: 'idle', message: 'Unsigned provider submit discarded. Reacquire the release graph before preparing another.' });
    } catch (error) {
      setState({ kind: 'refused', journal: state.journal, message: errorMessage(error) });
    }
  }

  const ready = wallet !== null && market.trim() !== '' && encodedVaa.trim() !== ''
    && postBody.trim() !== '' && lookupTable.trim() !== '' && reclaimAfter.trim() !== '';
  return <section className="workbench-actions">
    <header><span>Provider submission</span><h2>Post one verified update</h2><p>Create the Receiver update and its Resolution lifecycle in one table-backed, two-signer transaction.</p></header>
    <Label>Verified EncodedVaa<Input required value={encodedVaa} onChange={(event) => setEncodedVaa(event.target.value.trim())} placeholder="Router-verified EncodedVaa address" /></Label>
    <Label>Receiver PostUpdateParams body · base64<Input required value={postBody} onChange={(event) => setPostBody(event.target.value.trim())} placeholder="Body bytes without the Anchor discriminator" /></Label>
    <Label>Frozen provider lookup table<Input required value={lookupTable} onChange={(event) => setLookupTable(event.target.value.trim())} placeholder="Table containing the submit frame" /></Label>
    <Label>Earliest reclaim · Unix seconds<Input required inputMode="numeric" value={reclaimAfter} onChange={(event) => setReclaimAfter(event.target.value.trim())} placeholder="After the selected Window end" /></Label>
    <Card className="ready"><CardHeader><span className="operator-status ready-to-preflight">wallet submit</span><CardTitle>One message owns prepay and post</CardTitle><CardDescription>Rust authenticates the full Market→Source→Pyth graph and frozen table. A fresh in-memory update signs its writable slot; your wallet signs only as submitter and payer.</CardDescription></CardHeader><CardContent><dl className="operator-action-contract"><div><dt>Protocol accounts</dt><dd>38 exact</dd></div><div><dt>Signers</dt><dd>wallet + update</dd></div><div><dt>Completion</dt><dd>lifecycle + update</dd></div></dl></CardContent></Card>
    <Alert variant={state.kind === 'refused' ? 'destructive' : 'default'} aria-live="polite"><AlertTitle>{state.kind === 'refused' ? 'Refused safely' : 'Provider submit'}</AlertTitle><AlertDescription>{state.message}</AlertDescription></Alert>
    {(state.kind === 'idle' || state.kind === 'refused') && <Button type="button" disabled={!ready} onClick={() => void prepare()}>{wallet === null ? 'Connect the submitter wallet above' : 'Prepare exact provider submit'}</Button>}
    {state.kind === 'prepared' && <Button type="button" onClick={() => void signAndSubmit()}>Sign and submit provider update</Button>}
    {state.kind === 'signing' && <Button type="button" disabled>Waiting for wallet…</Button>}
    {state.kind === 'refused' && state.journal?.phase === 'unsigned' && <Button type="button" variant="outline" onClick={() => void discard()}>Discard unsigned saved submit</Button>}
    <footer><strong>Artifact sources</strong><span>The EncodedVaa is the Router’s verified account; the body is the Receiver PostUpdateParams payload without its discriminator; the table is the frozen provider table. The page reauthenticates all three against current finalized state.</span></footer>
  </section>;
}

function ProviderReclaimPanel({
  client,
  directory,
  endpoint,
  market,
  programs,
}: Readonly<{
  client: SolanaRpcClient;
  directory: ReturnType<typeof useWalletDirectoryV1>;
  endpoint: string;
  market: string;
  programs: Readonly<{ registryProgram: string; resolutionProgram: string }>;
}>) {
  const [lifecycle, setLifecycle] = useState('');
  const [state, setState] = useState<ProviderReclaimStateV1>({
    kind: 'idle',
    message: 'Paste the consumed provider lifecycle. Rust derives the reclaim and exact terminal balances.',
  });
  const wallet = directory.address;
  const wasmPromise = useMemo(() => loadSourceProviderWasmV1(), []);

  const poll = useCallback(async (
    journal: ClientOperationJournalV1,
    acquisition: SourceProviderReclaimAcquisitionV1 | null,
    alive: () => boolean = () => true,
  ) => {
    if (journal.phase !== 'submitted' || journal.signature === null) throw new Error('submitted provider recovery has no exact signature');
    const restored = await restoreSourceProviderJournalV1(journal);
    for (let attempt = 0; attempt < 30 && alive(); attempt += 1) {
      try {
        const status = (await client.signatureStatuses([journal.signature]))[0];
        if (!alive()) return;
        if (status?.known && status.succeeded === false) {
          setState({ kind: 'submitted', acquisition, journal, message: `The chain refused this exact reclaim (${status.errorText ?? 'unnamed chain error'}). Its signed record remains saved.` });
          return;
        }
        if (status?.known && status.confirmationStatus === 'finalized' && status.slot !== null
            && await sourceProviderReclaimPoststateCompletesV1(client, restored.rustPlan, status.slot)) {
          await clearFinalizedClientOperationJournalV1(browserStorage(), journal);
          setState({ kind: 'complete', message: 'Finalized: the lifecycle, Receiver update, and authority are closed, and the immutable refund recipient has the exact projected balance.' });
          return;
        }
        setState({ kind: 'submitted', acquisition, journal, message: 'Waiting for the exact signature and all four Rust-projected finalized poststates. Nothing is resubmitted.' });
      } catch (error) {
        if (alive()) setState({ kind: 'submitted', acquisition, journal, message: `${errorMessage(error)} The signed packet remains saved; reload only resumes its poststate check.` });
        return;
      }
      await pause();
    }
  }, [client]);

  useEffect(() => {
    if (wallet === null || market.trim() === '') return;
    let current = true;
    void (async () => {
      try {
        const admission = await client.assertMutationCluster();
        const journal = await findClientOperationJournalV1(browserStorage(), {
          clusterGenesis: admission.genesisHash,
          market: market.trim(),
          owner: wallet,
        }, 'source-provider-v1');
        if (!current || journal === null) return;
        if (journal.phase === 'submitted') {
          setState({ kind: 'submitted', acquisition: null, journal, message: 'Resuming the saved provider signature and exact finalized poststate check. Nothing is resubmitted.' });
          void poll(journal, null, () => current);
        } else {
          await restoreSourceProviderJournalV1(journal);
          setState({ kind: 'refused', journal, message: 'An unsigned reclaim is saved for this exact chain, Market, and wallet. Its operation-scoped resolver key was intentionally not persisted; discard it and prepare again.' });
        }
      } catch (error) {
        if (current) setState({ kind: 'refused', journal: null, message: `Provider recovery refused: ${errorMessage(error)}` });
      }
    })();
    return () => { current = false; };
  }, [client, market, poll, wallet]);

  async function prepare() {
    if (wallet === null || lifecycle.trim() === '' || market.trim() === '') return;
    setState({ kind: 'reading', message: 'Reacquiring the consumed lifecycle, terminal certificate, current programs, Pyth release, and four writable prestates…' });
    try {
      const admission = await client.assertMutationCluster();
      const acquisition = await acquireSourceProviderReclaimV1(
        client,
        await wasmPromise,
        lifecycle.trim(),
        wallet,
        programs,
      );
      if (acquisition.market !== market.trim()) throw new Error('provider lifecycle belongs to another Market');
      const input = await sourceProviderJournalInputV1({
        clusterGenesis: admission.genesisHash,
        market: acquisition.market,
        owner: wallet,
      }, acquisition);
      const journal = await writeUnsignedClientOperationJournalV1(browserStorage(), input);
      setState({ kind: 'prepared', acquisition, journal, message: 'Saved the exact unsigned Rust message. The next act signs with a fresh in-memory permissionless resolver, then asks this wallet only for the fee-payer signature.' });
    } catch (error) {
      setState({ kind: 'refused', journal: null, message: errorMessage(error) });
    }
  }

  async function signAndSubmit() {
    if (state.kind !== 'prepared' || wallet === null) return;
    const prepared = state;
    setState({ ...prepared, kind: 'signing', message: 'Signing with the operation-scoped resolver, then waiting for the wallet’s fee-payer signature…' });
    let submitted: ClientOperationJournalV1 | null = null;
    try {
      prepared.acquisition.transaction.sign([prepared.acquisition.resolver]);
      const signed = await requestWalletCosignTransactionV1(
        client,
        directory.handoff(endpoint),
        prepared.acquisition.transaction,
        wallet,
        prepared.acquisition.resolver.publicKey.toBase58(),
      );
      const signature = transactionSignatureV1(signed.transaction.signatures[0]!);
      submitted = await markClientOperationSubmittedV1(browserStorage(), prepared.journal, signature, signed.wireBytes);
      setState({ kind: 'submitted', acquisition: prepared.acquisition, journal: submitted, message: 'Saved the fully signed packet before transport; sending these exact bytes once.' });
      const returned = await submitSignedTransactionV1(client, submittedClientOperationWireV1(submitted));
      requireSubmittedSignatureMatchV1(signature, returned);
      await poll(submitted, prepared.acquisition);
    } catch (error) {
      setState(submitted === null
        ? { kind: 'refused', journal: prepared.journal, message: errorMessage(error) }
        : { kind: 'submitted', acquisition: prepared.acquisition, journal: submitted, message: `${errorMessage(error)} The exact signed packet remains saved and is never resubmitted automatically.` });
    }
  }

  async function discard() {
    if (state.kind !== 'refused' || state.journal?.phase !== 'unsigned') return;
    try {
      await discardUnsignedClientOperationJournalV1(browserStorage(), state.journal);
      setState({ kind: 'idle', message: 'Unsigned reclaim discarded. Read finalized state again before preparing another.' });
    } catch (error) {
      setState({ kind: 'refused', journal: state.journal, message: errorMessage(error) });
    }
  }

  return <section className="workbench-actions">
    <header><span>Provider cleanup</span><h2>Reclaim the consumed update</h2><p>Close the spent Receiver update and Resolution lifecycle, then return their exact rent to the immutable refund recipient.</p></header>
    <Label>Consumed provider lifecycle<Input required value={lifecycle} onChange={(event) => { setLifecycle(event.target.value.trim()); setState({ kind: 'idle', message: 'Lifecycle changed. Prepare from current finalized state.' }); }} placeholder="Resolution provider lifecycle address" /></Label>
    <Card className="ready"><CardHeader><span className="operator-status ready-to-preflight">permissionless reclaim</span><CardTitle>Wallet pays; fresh resolver authorizes</CardTitle><CardDescription>Rust derives all 18 accounts from the lifecycle and checked release. The resolver exists only in memory for this act; it never replaces wallet consent.</CardDescription></CardHeader><CardContent><dl className="operator-action-contract"><div><dt>Protocol accounts</dt><dd>18 exact</dd></div><div><dt>Wallet authority</dt><dd>fee payer only</dd></div><div><dt>Completion</dt><dd>4 exact poststates</dd></div></dl></CardContent></Card>
    <Alert variant={state.kind === 'refused' ? 'destructive' : 'default'} aria-live="polite"><AlertTitle>{state.kind === 'refused' ? 'Refused safely' : 'Provider reclaim'}</AlertTitle><AlertDescription>{state.message}</AlertDescription></Alert>
    {(state.kind === 'idle' || state.kind === 'refused') && <Button type="button" disabled={wallet === null || lifecycle.trim() === '' || market.trim() === ''} onClick={() => void prepare()}>{wallet === null ? 'Connect the payer wallet above' : market.trim() === '' ? 'Select the Market above' : 'Prepare exact reclaim'}</Button>}
    {state.kind === 'prepared' && <Button type="button" onClick={() => void signAndSubmit()}>Sign and submit reclaim</Button>}
    {state.kind === 'signing' && <Button type="button" disabled>Waiting for wallet…</Button>}
    {state.kind === 'refused' && state.journal?.phase === 'unsigned' && <Button type="button" variant="outline" onClick={() => void discard()}>Discard unsigned saved reclaim</Button>}
    <footer><strong>Safety contract</strong><span>The generated Rust/WASM owner rebuilds the lifecycle, Pyth record, ProgramData links, instruction, signer order, and exact four-account poststate. The browser saves before either signature, submits once, and clears only after finalized bytes match.</span></footer>
  </section>;
}

export default function ResolutionWorkspace() {
  const deployment = useDeploymentV1();
  const [endpoint, setEndpoint] = useDeploymentFieldV1((value) => value.endpoint);
  const [market, setMarket] = useState('');
  const [state, setState] = useState<ResolutionStateV1>({
    kind: 'idle',
    message: 'Paste one Market, then read its exact current Source funding route.',
  });
  const directory = useWalletDirectoryV1();
  const wallet = directory.address;
  const client = useMemo(() => new SolanaRpcClient(endpoint), [endpoint]);
  const wasmPromise = useMemo(() => loadSourceReadinessWasmV1(), []);
  const programs = useMemo(() => Object.freeze({
    coreProgram: deployment.programs.core,
    registryProgram: deployment.programs.registry,
    resolutionProgram: deployment.programs.resolution,
  }), [deployment]);

  const acquire = useCallback(async (wasm?: SourceReadinessWasmV1) => acquireSourceReadinessV1(
    client,
    wasm ?? await wasmPromise,
    market.trim(),
    programs,
  ), [client, market, programs, wasmPromise]);

  const pollSubmitted = useCallback(async (
    journal: ClientOperationJournalV1,
    before: SourceReadinessAcquisitionV1 | null,
    alive: () => boolean = () => true,
  ) => {
    if (journal.phase !== 'submitted' || journal.signature === null) throw new Error('submitted readiness recovery has no exact signature');
    const restored = await restoreSourceReadinessJournalV1(journal);
    for (let attempt = 0; attempt < 30 && alive(); attempt += 1) {
      try {
        const [status, after] = await Promise.all([
          client.signatureStatuses([journal.signature]).then((values) => values[0]),
          acquire(),
        ]);
        if (!alive()) return;
        if (status?.known && status.succeeded === false) {
          setState({ kind: 'submitted', acquisition: before, journal, message: `The chain reports an error (${status.errorText ?? 'unnamed chain error'}). The exact signed record remains saved; it cannot be replayed or discarded.` });
          return;
        }
        if (status?.known && status.confirmationStatus === 'finalized'
            && sourceReadinessPoststateCompletesV1(restored.intent.route, after.plan.route)) {
          await clearFinalizedClientOperationJournalV1(browserStorage(), journal);
          setState({ kind: 'complete', acquisition: after, signature: journal.signature, message: `Finalized at route ${after.plan.route}. The exact signed record is cleared.` });
          return;
        }
        setState({ kind: 'submitted', acquisition: before, journal, message: 'The exact signature or its adjacent finalized poststate is not visible yet. Reloading resumes this signature and never resubmits it.' });
      } catch (error) {
        if (alive()) setState({ kind: 'submitted', acquisition: before, journal, message: `${errorMessage(error)} The exact submitted record remains saved; reloading only resumes it.` });
        return;
      }
      await pause();
    }
    if (alive()) setState({ kind: 'submitted', acquisition: before, journal, message: 'Finalized completion is still unresolved. Reload later; the saved signature is never replayed.' });
  }, [acquire, client]);

  useEffect(() => {
    if (wallet === null || market.trim() === '') return;
    let current = true;
    void (async () => {
      try {
        const admission = await client.assertMutationCluster();
        const journal = await findClientOperationJournalV1(browserStorage(), {
          clusterGenesis: admission.genesisHash,
          market: market.trim(),
          owner: wallet,
        }, 'source-readiness-v1');
        if (!current || journal === null) return;
        if (journal.phase === 'submitted') {
          setState({ kind: 'submitted', acquisition: null, journal, message: 'Resuming the exact saved signature and finalized poststate check. Nothing is resubmitted.' });
          void pollSubmitted(journal, null, () => current);
        } else {
          await restoreSourceReadinessJournalV1(journal);
          setState({ kind: 'refused', journal, message: 'This browser retains an unsigned readiness packet for this exact chain, Market, and wallet. Discard it explicitly, then read current finalized state before preparing another.' });
        }
      } catch (error) {
        if (current) setState({ kind: 'refused', journal: null, message: `Recovery refused: ${errorMessage(error)}` });
      }
    })();
    return () => { current = false; };
  }, [client, market, pollSubmitted, wallet]);

  async function read() {
    setState({ kind: 'reading', message: 'Verifying the Rust planner blob and reading one exact finalized account frame…' });
    try {
      const acquisition = await acquire();
      const copy = routeCopy(acquisition.plan.route);
      setState(acquisition.plan.route === 'complete' || acquisition.plan.route === 'consumed-by-founding'
        ? { kind: 'complete', acquisition, signature: null, message: copy.outcome }
        : { kind: 'observed', acquisition, message: copy.outcome });
    } catch (error) {
      setState({ kind: 'refused', journal: null, message: errorMessage(error) });
    }
  }

  async function prepare() {
    if (state.kind !== 'observed' || wallet === null) return;
    setState({ kind: 'reading', message: 'Rechecking mutation-cluster identity, current blockhash, and durable recovery storage…' });
    try {
      const acquisition = await acquire();
      if (acquisition.plan.route !== state.acquisition.plan.route
          || (acquisition.plan.route !== 'create' && acquisition.plan.route !== 'activate' && acquisition.plan.route !== 'accept')) {
        throw new Error(`finalized readiness moved from ${state.acquisition.plan.route} to ${acquisition.plan.route}; inspect the new route before preparing a wallet act`);
      }
      const admission = await client.assertMutationCluster();
      const blockhash = await client.latestMutationBlockhash(acquisition.plan.observedSlot);
      const transaction = buildSourceReadinessTransactionV1(acquisition, wallet, blockhash);
      const input = await sourceReadinessJournalInputV1({
        clusterGenesis: admission.genesisHash,
        market: market.trim(),
        owner: wallet,
      }, acquisition, transaction);
      const journal = await writeUnsignedClientOperationJournalV1(browserStorage(), input);
      setState({ kind: 'prepared', acquisition, transaction, journal, message: 'Saved the exact unsigned plan. The next button makes one wallet signature request.' });
    } catch (error) {
      setState({ kind: 'refused', journal: null, message: errorMessage(error) });
    }
  }

  async function signAndSubmit() {
    if (state.kind !== 'prepared' || wallet === null) return;
    const prepared = state;
    setState({ ...prepared, kind: 'signing', message: 'Waiting for the wallet to sign the exact sole-payer packet…' });
    let submitted: ClientOperationJournalV1 | null = null;
    try {
      const signed = await requestWalletTransactionSignatureV1(client, directory.handoff(endpoint), prepared.transaction.transaction, wallet);
      if (!signed.complete) throw new Error('the wallet did not complete the sole required payer signature');
      const signature = transactionSignatureV1(signed.transaction.signatures[0]!);
      submitted = await markClientOperationSubmittedV1(browserStorage(), prepared.journal, signature, signed.wireBytes);
      setState({ kind: 'submitted', acquisition: prepared.acquisition, journal: submitted, message: 'Saved the exact signed packet before submission; sending it once with preflight…' });
      const returned = await submitSignedTransactionV1(client, submittedClientOperationWireV1(submitted));
      requireSubmittedSignatureMatchV1(signature, returned);
      await pollSubmitted(submitted, prepared.acquisition);
    } catch (error) {
      if (submitted !== null) setState({ kind: 'submitted', acquisition: prepared.acquisition, journal: submitted, message: `${errorMessage(error)} The signed record remains saved and will never be resubmitted automatically.` });
      else setState({ kind: 'refused', journal: prepared.journal, message: errorMessage(error) });
    }
  }

  async function discard(journal: ClientOperationJournalV1) {
    try {
      await discardUnsignedClientOperationJournalV1(browserStorage(), journal);
      setState({ kind: 'idle', message: 'Unsigned saved plan discarded. Read current finalized state before preparing another.' });
    } catch (error) {
      setState({ kind: 'refused', journal, message: errorMessage(error) });
    }
  }

  const acquisition = state.kind === 'observed' || state.kind === 'prepared' || state.kind === 'signing'
    || state.kind === 'complete' ? state.acquisition : state.kind === 'submitted' ? state.acquisition : null;
  const copy = acquisition === null ? null : routeCopy(acquisition.plan.route);
  const unsignedJournal = state.kind === 'prepared' || state.kind === 'signing'
    ? state.journal
    : state.kind === 'refused' && state.journal?.phase === 'unsigned' ? state.journal : null;

  return <main className="product-shell workbench-shell">
    <ConsoleHeader path="/resolution" title="Resolution funding" purpose="Finish the Market’s exact Source funding-readiness walk." />
    <section className="workbench-heading"><div><h1>Make resolution<br />ready.</h1></div><p>Read the current Market, authenticate its releases and records in the Rust planner, then execute the one adjacent permissionless act. The browser owns RPC, durable recovery, wallet consent, submission, and finalized poststate verification.</p></section>

    <div className="workbench-grid">
      <section className="workbench-coordinates">
        <header><span>Exact chain inputs</span><h2>Select one Market</h2><p>{deployment.label} supplies the checked Core, Registry, and Resolution programs. The Market supplies every child coordinate and funding entry.</p></header>
        <Label>Finalized RPC endpoint<Input type="url" required value={endpoint} onChange={(event) => setEndpoint(event.target.value.trim())} /></Label>
        <Label>Core Market<Input required value={market} onChange={(event) => { setMarket(event.target.value.trim()); setState({ kind: 'idle', message: 'Market changed. Read its current finalized route.' }); }} placeholder="Canonical Market address" /></Label>
        <details className="operator-override"><summary>Programs · filled from {deployment.label}</summary><dl className="workbench-authority"><div><dt>Core</dt><dd>{short(programs.coreProgram)}</dd></div><div><dt>Registry</dt><dd>{short(programs.registryProgram)}</dd></div><div><dt>Resolution</dt><dd>{short(programs.resolutionProgram)}</dd></div></dl></details>
        <Button type="button" onClick={() => void read()} disabled={market.trim() === '' || state.kind === 'reading'}>{state.kind === 'reading' ? 'Reading finalized state…' : 'Read exact readiness route'}</Button>
        <Alert variant={state.kind === 'refused' ? 'destructive' : 'default'} aria-live="polite"><AlertTitle>{state.kind === 'refused' ? 'Refused safely' : 'Current status'}</AlertTitle><AlertDescription>{state.message}</AlertDescription></Alert>
      </section>

      <section className="workbench-actions">
        <header><span>Current adjacent act</span><h2>{copy?.title ?? 'No route read yet'}</h2><p>{copy?.outcome ?? 'No sample or projection is shown. Read one Market to select a real route.'}</p></header>
        {acquisition !== null && <Card className="ready"><CardHeader><span className="operator-status ready-to-preflight">{acquisition.plan.route}</span><CardTitle>{copy?.title}</CardTitle><CardDescription>{copy?.authority}</CardDescription></CardHeader><CardContent><dl className="operator-action-contract"><div><dt>Finalized slot</dt><dd>{acquisition.plan.observedSlot}</dd></div><div><dt>Protocol accounts</dt><dd>{acquisition.plan.geometry?.protocolAccountCount ?? 0}</dd></div><div><dt>Protocol signers</dt><dd>{acquisition.plan.geometry?.protocolSignerCount ?? 0}</dd></div><div><dt>Exact prepay</dt><dd>{acquisition.plan.prepay?.lamports ?? '0'} lamports</dd></div></dl></CardContent></Card>}
        <WalletDirectory directory={directory} onConnected={() => setState((current) => current)} />
        {state.kind === 'observed' && copy?.button != null && <Button type="button" disabled={wallet === null} onClick={() => void prepare()}>{wallet === null ? 'Connect a payer wallet first' : copy.button}</Button>}
        {state.kind === 'prepared' && <Button type="button" onClick={() => void signAndSubmit()}>Sign and submit {state.transaction.route}</Button>}
        {state.kind === 'signing' && <Button type="button" disabled>Waiting for wallet…</Button>}
        {unsignedJournal !== null && state.kind !== 'signing' && <Button type="button" variant="outline" onClick={() => void discard(unsignedJournal)}>Discard unsigned saved plan</Button>}
        <footer><strong>Safety contract</strong><span>The Rust/WASM owner derives every coordinate and instruction. The page verifies the blob, requires one finalized observation, saves before signing and before submission, sends once, and clears recovery only after the next exact route is finalized.</span></footer>
      </section>
    </div>
    <section className="workbench-heading"><div><h2>Admit the<br />terminal result.</h2></div><p>Once provider evidence has produced a terminal Source and certificate, bind that exact selector into Core. This is permissionless; the connected wallet only pays the transaction fee.</p></section>
    <div className="workbench-grid">
      <section className="workbench-coordinates"><header><span>Admission authority</span><h2>Use the Market and wallet above</h2><p>The Market selects its Product, releases, funding set, and Source. The Source selects the certificate and terminal sequence; neither is entered here.</p></header><dl className="workbench-authority"><div><dt>Market</dt><dd>{market.trim() === '' ? 'select above' : short(market.trim())}</dd></div><div><dt>Wallet</dt><dd>{wallet === null ? 'connect above' : short(wallet)}</dd></div><div><dt>Core</dt><dd>{short(programs.coreProgram)}</dd></div></dl></section>
      <SourceTerminalPanel client={client} directory={directory} endpoint={endpoint} market={market} programs={programs} wasmPromise={wasmPromise} />
    </div>
    <section className="workbench-heading"><div><h2>Discharge the<br />Source fund.</h2></div><p>After Core enters Retiring, fund the durable closure receipt exactly once and execute the permissionless V7 direct close. Source principal, ledger rent, and surplus return only to the immutable beneficiary.</p></section>
    <div className="workbench-grid">
      <section className="workbench-coordinates"><header><span>Close authority</span><h2>Use the Market and wallet above</h2><p>The Retiring Market and terminal Source derive every coordinate. The wallet only pays receipt rent and transaction fees.</p></header><dl className="workbench-authority"><div><dt>Market</dt><dd>{market.trim() === '' ? 'select above' : short(market.trim())}</dd></div><div><dt>Wallet</dt><dd>{wallet === null ? 'connect above' : short(wallet)}</dd></div><div><dt>Resolution</dt><dd>{short(programs.resolutionProgram)}</dd></div></dl></section>
      <SourceCloseFundPanel client={client} directory={directory} endpoint={endpoint} market={market} programs={programs} wasmPromise={wasmPromise} />
    </div>
    <section className="workbench-heading"><div><h2>Post provider<br />evidence.</h2></div><p>Join a Router-verified VAA and exact Receiver body to this Market’s current Source and Pyth release graph, then create the update and its reclaimable lifecycle atomically.</p></section>
    <div className="workbench-grid">
      <section className="workbench-coordinates"><header><span>Submit authority</span><h2>Use the Market and wallet above</h2><p>The Market selects the records and immutable refund recipient. The connected wallet must already be the EncodedVaa write authority and pays the lifecycle top-up.</p></header><dl className="workbench-authority"><div><dt>Market</dt><dd>{market.trim() === '' ? 'select above' : short(market.trim())}</dd></div><div><dt>Wallet</dt><dd>{wallet === null ? 'connect above' : short(wallet)}</dd></div><div><dt>Resolution</dt><dd>{short(programs.resolutionProgram)}</dd></div></dl></section>
      <ProviderSubmitPanel client={client} directory={directory} endpoint={endpoint} market={market} programs={programs} />
    </div>
    <section className="workbench-heading"><div><h2>Close provider<br />work.</h2></div><p>A consumed real-provider update has one permissionless cleanup act. The lifecycle itself supplies the immutable Market, release, refund, and terminal-certificate coordinates.</p></section>
    <div className="workbench-grid">
      <section className="workbench-coordinates"><header><span>Reclaim input</span><h2>Use the Market above</h2><p>The reclaim must join the lifecycle back to that exact Market and the current Registry and Resolution deployment.</p></header><dl className="workbench-authority"><div><dt>Market</dt><dd>{market.trim() === '' ? 'select above' : short(market.trim())}</dd></div><div><dt>Registry</dt><dd>{short(programs.registryProgram)}</dd></div><div><dt>Resolution</dt><dd>{short(programs.resolutionProgram)}</dd></div></dl></section>
      <ProviderReclaimPanel client={client} directory={directory} endpoint={endpoint} market={market} programs={{ registryProgram: programs.registryProgram, resolutionProgram: programs.resolutionProgram }} />
    </div>
  </main>;
}
