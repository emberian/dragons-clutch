'use client';

import PageShell from '@/components/PageShell';
import ConsoleHeader from '@/components/ConsoleHeader';
import { FormEvent, useState } from 'react';

import {
  type BearerTransferInspectionV2,
  type BearerTransferPlanV2,
  buildUnsignedBearerTransferV2,
  inspectBearerTransferV2,
  tokenBehaviorSummaryV2,
  verifyBearerTransferFinalizedPoststateV2,
} from '@dclutch/sdk/rationalTokenV2';
import { SolanaRpcClient } from '@dclutch/sdk/rpc';
import {
  type WalletSignedTransactionV1,
  requestWalletAddTransactionSignatureV1,
  submitSignedTransactionV1,
} from '@dclutch/sdk/walletHandoff';
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
import { bearerTransferJournalInputV2, nextBearerTransferSignerV2, restoreBearerTransferJournalV2 } from '@/lib/bearerTransferOperationV2';

import WalletDirectory, { useWalletDirectoryV1 } from './WalletDirectory';
import RationalRetireReceiptPanel from './RationalRetireReceiptPanel';
import RationalOpenPanel from './RationalOpenPanel';
import RationalTerminalPanel from './RationalTerminalPanel';
import { useDeploymentFieldV1 } from '@/lib/deploymentStore';

type InspectionState = Readonly<{ kind: 'idle' | 'loading' | 'refused'; message: string }>
  | Readonly<{ kind: 'ready'; message: string; inspection: BearerTransferInspectionV2 }>;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'operation refused without a usable reason';
}

function parseRawU64(value: string): bigint {
  if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new Error('raw quantity must be canonical unsigned decimal atoms');
  const amount = BigInt(value);
  if (amount === 0n || amount > 18_446_744_073_709_551_615n) throw new Error('raw quantity must be 1..u64::MAX atoms');
  return amount;
}

function base64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

function short(value: string): string {
  return value.length <= 20 ? value : `${value.slice(0, 10)}…${value.slice(-8)}`;
}

function browserStorage(): Storage {
  if (typeof window === 'undefined' || window.localStorage === undefined) {
    throw new Error('this browser does not expose local recovery storage, so no wallet signature was requested');
  }
  return window.localStorage;
}

type TransferCompletion = Readonly<{
  signature: string;
  observedSlot: string;
  sourceAfter: bigint;
  destinationAfter: bigint;
}>;

export default function RationalRepresentationWorkspace() {
  const [endpoint, setEndpoint] = useDeploymentFieldV1((d) => d.endpoint);
  const [payer, setPayer] = useState('');
  const [authority, setAuthority] = useState('');
  const [coreProgram, setCoreProgram] = useDeploymentFieldV1((d) => d.programs.core);
  const [market, setMarket] = useState('');
  const [mint, setMint] = useState('');
  const [source, setSource] = useState('');
  const [destination, setDestination] = useState('');
  const [lookupTable, setLookupTable] = useState('');
  const [rawAmount, setRawAmount] = useState('');
  const [state, setState] = useState<InspectionState>({ kind: 'idle', message: 'No Market or Token-2022 state has been read.' });
  const [plan, setPlan] = useState<BearerTransferPlanV2 | null>(null);
  const [buildStatus, setBuildStatus] = useState('Authenticate one finalized TokenBehaviorSelectionV2 route first.');
  const [wallet, setWallet] = useState('');
  const [walletStatus, setWalletStatus] = useState('No wallet identity has been requested.');
  const wallets = useWalletDirectoryV1();
  const [signed, setSigned] = useState<WalletSignedTransactionV1 | null>(null);
  const [lastValidBlockHeight, setLastValidBlockHeight] = useState<string | null>(null);
  const [journal, setJournal] = useState<ClientOperationJournalV1 | null>(null);
  const [submitStatus, setSubmitStatus] = useState('No signed packet has been submitted.');
  const [completion, setCompletion] = useState<TransferCompletion | null>(null);
  const inspection = state.kind === 'ready' ? state.inspection : null;
  const summary = inspection === null ? null : tokenBehaviorSummaryV2(inspection);

  async function inspect(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setPlan(null); setSigned(null); setJournal(null); setLastValidBlockHeight(null); setCompletion(null);
    setState({ kind: 'loading', message: 'Deriving the behavior record from finalized Market state and reacquiring the Mint, Token Accounts, and ALT…' });
    try {
      const next = await inspectBearerTransferV2(new SolanaRpcClient(endpoint), {
        payer, authority, coreProgram, market, mint, source, destination, lookupTable,
      });
      setState({
        kind: 'ready', inspection: next,
        message: `Exact TokenBehaviorSelectionV2 and extension-safe Token-2022 route joined at finalized slot ${next.observedSlot}.`,
      });
      const client = new SolanaRpcClient(endpoint);
      const admission = await client.probe();
      const saved = await findClientOperationJournalV1(browserStorage(), {
        clusterGenesis: admission.genesisHash, market: next.market, owner: next.payer,
      }, 'bearer-transfer-v2');
      if (saved !== null) {
        const restored = await restoreBearerTransferJournalV2(saved);
        if (restored.poststate.authority !== next.authority || restored.poststate.mint !== next.mint.mint
            || restored.poststate.source !== next.source.address || restored.poststate.destination !== next.destination.address) {
          throw new Error('saved Bearer transfer coordinates differ from the authenticated route');
        }
        setJournal(saved);
        if (saved.phase === 'submitted') {
          setSubmitStatus('Resuming the saved signature and finalized balance check. Nothing is resubmitted.');
          await pollSubmitted(saved);
        } else {
          setSubmitStatus('An unsigned transfer plan is saved for this payer and Market. Discard it explicitly before building a different packet.');
        }
      }
    } catch (error) {
      setState({ kind: 'refused', message: `Refused: ${errorMessage(error)}` });
    }
  }

  async function build() {
    if (inspection === null) return;
    setPlan(null); setSigned(null); setJournal(null); setCompletion(null);
    try {
      const client = new SolanaRpcClient(endpoint);
      const blockhash = await client.latestMutationBlockhash(inspection.observedSlot);
      const next = buildUnsignedBearerTransferV2(inspection, blockhash.blockhash, parseRawU64(rawAmount));
      setPlan(next);
      setLastValidBlockHeight(blockhash.lastValidBlockHeight);
      setBuildStatus(`Unsigned v0 TransferChecked packet: ${next.wireBytes.length} / 1232 bytes · ${next.loadedAddresses} ALT addresses · ${next.rawAmount.toString()} raw atoms.`);
    } catch (error) {
      setBuildStatus(`Refused: ${errorMessage(error)}`);
    }
  }

  function adoptIdentity(address: string) {
    setWallet(address);
    if (payer === '') setPayer(address);
    if (authority === '') setAuthority(address);
    setWalletStatus(`Connected · ${address}`);
  }

  async function signTransaction() {
    if (plan === null || lastValidBlockHeight === null) return;
    try {
      const client = new SolanaRpcClient(endpoint);
      const admission = await client.assertMutationCluster();
      const height = BigInt(await client.blockHeight());
      if (height > BigInt(lastValidBlockHeight)) throw new Error(`packet expired at block height ${lastValidBlockHeight}; rebuild it before signing`);
      const current = signed?.transaction ?? plan.transaction;
      const expectedSigner = nextBearerTransferSignerV2(current, payer, authority);
      if (expectedSigner === null) throw new Error('every required signature is already present');
      if (wallet !== expectedSigner.address) throw new Error(`connect the ${expectedSigner.role} wallet ${expectedSigner.address}`);
      const retained = journal ?? await writeUnsignedClientOperationJournalV1(browserStorage(),
        await bearerTransferJournalInputV2({ clusterGenesis: admission.genesisHash, market: plan.poststate.market, owner: payer }, plan, lastValidBlockHeight));
      setJournal(retained);
      const next = await requestWalletAddTransactionSignatureV1(client, wallets.handoff(endpoint), current, expectedSigner.address);
      setSigned(next);
      setWalletStatus(next.complete
        ? 'Every required wallet signature is valid over the exact packet. Nothing has been submitted.'
        : `The transfer authority signed without changing the packet. Connect the transaction payer ${payer} to finish.`);
    } catch (error) {
      setWalletStatus(`Refused: ${errorMessage(error)}`);
    }
  }

  async function pollSubmitted(saved: ClientOperationJournalV1): Promise<void> {
    if (saved.phase !== 'submitted' || saved.signature === null) throw new Error('Bearer transfer recovery requires one submitted signature');
    const restored = await restoreBearerTransferJournalV2(saved);
    const client = new SolanaRpcClient(endpoint);
    for (let attempt = 0; attempt < 30; attempt += 1) {
      try {
        const status = (await client.signatureStatuses([saved.signature]))[0];
        if (status?.known && status.succeeded === false) {
          setSubmitStatus(`The chain reports an error (${status.errorText ?? 'unnamed chain error'}). The saved record remains because this packet cannot be replayed.`);
          return;
        }
        if (status?.known && status.succeeded === true && status.confirmationStatus === 'finalized') {
          const verified = await verifyBearerTransferFinalizedPoststateV2(client, restored.poststate, status.slot ?? undefined);
          await clearFinalizedClientOperationJournalV1(browserStorage(), saved);
          setCompletion({ signature: saved.signature, observedSlot: verified.observedSlot,
            sourceAfter: verified.poststate.sourceAfter, destinationAfter: verified.poststate.destinationAfter });
          setJournal(null);
          setSubmitStatus(`Finalized at slot ${verified.observedSlot}; exact source and destination balances now match the transfer.`);
          return;
        }
        setSubmitStatus('The saved signature is not finalized yet. You can close this page; authenticating the same route resumes it without sending again.');
      } catch (error) {
        setSubmitStatus(`${errorMessage(error)} The saved signature remains; recovery never resubmits it.`);
        return;
      }
      await new Promise<void>((resolve) => setTimeout(resolve, 1_500));
    }
  }

  async function submitTransaction() {
    if (plan === null || signed?.complete !== true || journal === null || lastValidBlockHeight === null) return;
    let submitted: ClientOperationJournalV1 | null = null;
    const signature = transactionSignatureV1(signed.transaction.signatures[0]!);
    try {
      const client = new SolanaRpcClient(endpoint);
      const admission = await client.assertMutationCluster();
      if (admission.genesisHash !== journal.clusterGenesis) throw new Error('RPC genesis changed after signing');
      if (BigInt(await client.blockHeight()) > BigInt(lastValidBlockHeight)) throw new Error(`signed packet expired at block height ${lastValidBlockHeight}`);
      submitted = await markClientOperationSubmittedV1(browserStorage(), journal, signature, signed.wireBytes);
      setJournal(submitted); setSubmitStatus('Saved before submission; sending the exact signed packet once…');
      const returned = await submitSignedTransactionV1(client, submittedClientOperationWireV1(submitted));
      requireSubmittedSignatureMatchV1(signature, returned);
      await pollSubmitted(submitted);
    } catch (error) {
      setSubmitStatus(`${errorMessage(error)}${submitted === null ? '' : ' The submitted record stays saved; recovery never resubmits it.'}`);
    }
  }

  async function discardUnsignedPlan() {
    if (journal?.phase !== 'unsigned') return;
    try {
      await discardUnsignedClientOperationJournalV1(browserStorage(), journal);
      setJournal(null); setSigned(null); setPlan(null); setLastValidBlockHeight(null);
      setSubmitStatus('The unsigned saved plan was discarded. No transaction was submitted.');
    } catch (error) {
      setSubmitStatus(`Refused: ${errorMessage(error)}`);
    }
  }

  function downloadPacket() {
    if (plan === null) return;
    const wire = signed?.wireBytes ?? plan.wireBytes;
    const blob = new Blob([wire as BlobPart], { type: 'application/octet-stream' });
    const link = document.createElement('a');
    link.href = URL.createObjectURL(blob);
    link.download = `dclutch-bearer-transfer-${plan.rawAmount.toString()}-raw-${signed === null ? 'unsigned' : 'wallet-signed'}-${wire.length}.bin`;
    link.click(); URL.revokeObjectURL(link.href);
  }

  return <PageShell className="product-shell trade-v3-shell" header={<ConsoleHeader path="/representation" title="Representation" purpose="Transfer bearer claims and inspect representation lifecycle routes on a local or compatible custom chain." />}>

    <section className="trade-v3-hero"><div><h1>Claims &amp;<br /><em>redemption.</em></h1><p>The constructors below have local execution evidence and derive their routes from on-chain state. No current devnet market can supply that route today. Opening a representation and retiring a receipt produce unsigned candidates; where a step is unavailable, the console says exactly what is missing.</p></div><aside><span>Local/custom chain</span><strong>Bearer transfer</strong><p>No current devnet market can use this surface. Terminal redemption is SBF-tested, while browser payout remains read-only until it consumes the canonical Rust emitter.</p></aside></section>

    <section className="trade-v3-card">
      <header><span>00</span><div><h2>Successor route truth, without pretending incomplete Hot paths execute</h2><p>Transfer is the normal Token-2022 instruction. Wrap/open/redeem/retire are distinct privileged lifecycle actions; this interface will expose them only from their finalized SetV2/CapabilityV4 operators.</p></div></header>
      <div className="trade-v3-evidence"><article><span>Bearer transfer</span><strong>transaction-complete</strong><small>exact TransferChecked · v0 + ALT</small></article><article><span>Rational open</span><strong>chain-derived</strong><small>four CapabilityV4 actions · packet bounded</small></article><article><span>Terminal redeem</span><strong>SBF-tested</strong><small>browser payout projection · Rust-emitter gated</small></article><article><span>Receipt retirement</span><strong>packet-complete</strong><small>closure only · signing release-gated</small></article></div>
    </section>

    <form className="trade-v3-card route-card" onSubmit={(event) => void inspect(event)}>
      <header><span>01</span><div><h2>Derive and authenticate one transfer route from chain state</h2><p>The selection record address is not caller input. It is derived from CoreStateV2&apos;s immutable Realm and release-set identities, then checked as exact finalized Registry content. Mint and holder state come only from finalized RPC accounts.</p></div></header>
      <div className="direct-form-grid"><label><span>Finalized RPC endpoint</span><input type="url" required value={endpoint} onChange={(event) => setEndpoint(event.target.value.trim())} /></label><label><span>Core program</span><input required value={coreProgram} onChange={(event) => setCoreProgram(event.target.value.trim())} /></label><label><span>Market CoreStateV2</span><input required value={market} onChange={(event) => setMarket(event.target.value.trim())} /></label><label><span>Transaction payer</span><input required value={payer} onChange={(event) => setPayer(event.target.value.trim())} /></label><label><span>Source transfer authority</span><input required value={authority} onChange={(event) => setAuthority(event.target.value.trim())} /></label><label><span>Claim Mint</span><input required value={mint} onChange={(event) => setMint(event.target.value.trim())} /></label><label><span>Source Token Account</span><input required value={source} onChange={(event) => setSource(event.target.value.trim())} /></label><label><span>Destination Token Account</span><input required value={destination} onChange={(event) => setDestination(event.target.value.trim())} /></label><label><span>Address lookup table</span><input required value={lookupTable} onChange={(event) => setLookupTable(event.target.value.trim())} /></label></div>
      <button disabled={state.kind === 'loading'}>{state.kind === 'loading' ? 'Reading finalized representation state…' : 'Authenticate exact transfer route'}</button><p className="direct-status" aria-live="polite">{state.message}</p>
      {inspection && summary && <div className="trade-v3-evidence"><article><span>Market / generation</span><strong>{short(inspection.market)} · {inspection.generation.toString()}</strong><small>{inspection.marketPhase}</small></article><article><span>Behavior selection</span><strong>{summary.selectionDigest.slice(0, 16)}…</strong><small>Registry record {short(inspection.selectionRecord)}</small></article><article><span>Raw source balance</span><strong>{inspection.source.rawAmount.toString()} atoms</strong><small>no display conversion</small></article><article><span>Display metadata</span><strong>{inspection.mint.displayDecimals} decimals</strong><small>Mint supply {inspection.mint.rawSupply.toString()} raw atoms</small></article></div>}
    </form>

    <section className="trade-v3-card">
      <header><span>02</span><div><h2>Construct one exact unsigned v0 transfer</h2><p>Enter raw base-unit atoms. Even when the Mint advertises 255 decimals, this field remains a canonical u64 integer; the decimals byte is copied only into Token-2022&apos;s checked instruction.</p></div></header>
      <div className="direct-form-grid"><label><span>Raw u64 atoms · never UI units</span><input inputMode="numeric" required value={rawAmount} onChange={(event) => setRawAmount(event.target.value.trim())} /></label></div>
      <button type="button" disabled={inspection === null} onClick={() => void build()}>Build exact unsigned v0 + ALT packet</button><p className="direct-status" aria-live="polite">{buildStatus}</p>
      {plan && <div className="direct-output"><dl><div><dt>Raw transfer</dt><dd>{plan.rawAmount.toString()} atoms</dd></div><div><dt>Display metadata only</dt><dd>{plan.displayDecimals} decimals · no exponentiation or rounding performed</dd></div><div><dt>Packet</dt><dd>{plan.wireBytes.length} / 1232 bytes · {plan.loadedAddresses} ALT addresses</dd></div><div><dt>Required signers</dt><dd>{plan.requiredSigners.join(', ')}</dd></div></dl></div>}
    </section>

    <section className="trade-v3-card signing-card">
      <header><span>03</span><div><h2>Sign with each required wallet, then submit once</h2><p>Connecting reads identity only. The source authority signs first; when the payer is distinct, connect that wallet next. Each wallet may fill only its own slot, and every earlier signature is verified and preserved.</p></div></header>
      <WalletDirectory directory={wallets} onConnected={adoptIdentity} />
      <div className="signing-grid"><article><span>Wallet identity</span><strong>{wallet || 'not connected'}</strong><p>{walletStatus}</p></article><article><span>Unsigned / signed packet</span><strong>{plan ? `${plan.wireBytes.length} bytes · ${plan.loadedAddresses} ALT` : 'no packet built'}</strong><button type="button" disabled={plan === null || signed?.complete === true} onClick={() => void signTransaction()}>{signed === null ? 'Sign as transfer authority' : signed.complete ? 'All signatures complete' : 'Sign as transaction payer'}</button><button type="button" disabled={plan === null} onClick={downloadPacket}>Download exact packet</button><button type="button" disabled={signed?.complete !== true || journal === null || journal.phase === 'submitted'} onClick={() => void submitTransaction()}>Submit fully signed transfer</button><button type="button" disabled={journal?.phase !== 'unsigned'} onClick={() => void discardUnsignedPlan()}>Discard unsigned saved plan</button><p>No automatic submission or hidden retry. {submitStatus}</p></article></div>
      {plan && <div className="direct-output"><dl><div><dt>Expected source</dt><dd>{plan.poststate.sourceBefore.toString()} → {plan.poststate.sourceAfter.toString()} raw atoms</dd></div><div><dt>Expected destination</dt><dd>{plan.poststate.destinationBefore.toString()} → {plan.poststate.destinationAfter.toString()} raw atoms</dd></div></dl></div>}
      {completion && <div className="direct-output"><dl><div><dt>Verified finalized balances</dt><dd>source {completion.sourceAfter.toString()} raw atoms · destination {completion.destinationAfter.toString()} raw atoms</dd></div><div><dt>Transaction</dt><dd>slot {completion.observedSlot} · signature {short(completion.signature)}</dd></div></dl></div>}
      {plan && <details className="trade-v3-bytes"><summary>Exact transfer material</summary><dl><div><dt>Instruction bytes · base64</dt><dd>{base64(plan.instructionBytes)}</dd></div><div><dt>Packet bytes · base64</dt><dd>{base64(signed?.wireBytes ?? plan.wireBytes)}</dd></div></dl></details>}
    </section>

    <RationalOpenPanel />

    <RationalTerminalPanel />

    <RationalRetireReceiptPanel />

    <footer className="product-footer"><span>Arbitrary u8 display decimals · exact raw-u64 economics</span><span>No mock token state · no hidden rounding · one saved send, finalized balance proof</span></footer>
  </PageShell>;
}
