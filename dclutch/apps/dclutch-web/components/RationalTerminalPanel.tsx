'use client';

import { FormEvent, useState } from 'react';

import {
  inspectRationalTerminalReadinessV4,
  rationalTerminalReadinessSummaryV4,
  type RationalTerminalReadinessV4,
} from '@/lib/rationalTerminalChainV4';
import { SolanaRpcClient } from '@/lib/rpc';

import WalletDirectory, { useWalletDirectoryV1 } from './WalletDirectory';

type State = Readonly<{ kind: 'idle' | 'loading' | 'refused'; message: string }>
  | Readonly<{ kind: 'ready'; message: string; inspection: RationalTerminalReadinessV4 }>;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'operation refused without a usable reason';
}

function rawU64(text: string): bigint {
  if (!/^[1-9][0-9]*$/.test(text)) throw new Error('raw quantity must be canonical positive decimal units');
  const value = BigInt(text);
  if (value > 18_446_744_073_709_551_615n) throw new Error('raw quantity exceeds u64::MAX');
  return value;
}

function outcome(text: string): number {
  if (!/^(0|[1-9][0-9]*)$/.test(text)) throw new Error('selected claim must be one canonical u32 index');
  const value = Number(text);
  if (!Number.isSafeInteger(value) || value > 0xffff_ffff) throw new Error('selected claim exceeds u32::MAX');
  return value;
}

function fixedAddresses(text: string): string[] {
  const addresses = text.split(/\r?\n/).map((line) => line.trim()).filter((line) => line.length > 0);
  if (addresses.length !== 38) throw new Error(`Hot frame needs exactly 38 address lines; received ${addresses.length}`);
  return addresses;
}

export default function RationalTerminalPanel() {
  const [endpoint, setEndpoint] = useState('http://127.0.0.1:8899');
  const [payer, setPayer] = useState(''); const [actor, setActor] = useState('');
  const [descriptor, setDescriptor] = useState(''); const [lookupTable, setLookupTable] = useState('');
  const [fixed, setFixed] = useState(''); const [selected, setSelected] = useState('0'); const [quantity, setQuantity] = useState('');
  const [walletStatus, setWalletStatus] = useState('No wallet identity has been requested.');
  const wallets = useWalletDirectoryV1();
  const [state, setState] = useState<State>({ kind: 'idle', message: 'No terminal Product/representation state has been read.' });
  const inspection = state.kind === 'ready' ? state.inspection : null;
  const summary = inspection === null ? null : rationalTerminalReadinessSummaryV4(inspection);

  function adoptIdentity(address: string) {
    if (payer === '') setPayer(address); if (actor === '') setActor(address);
    setWalletStatus(`${address} · identity only; no transaction or signature request`);
  }

  async function inspect(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setState({ kind: 'loading', message: 'Reacquiring terminal Core, CapabilityV4, Product N, representation K, ProductBasisV3, and exact terminal coordinate…' });
    try {
      const next = await inspectRationalTerminalReadinessV4(new SolanaRpcClient(endpoint), {
        payer, actor, descriptorId: descriptor, lookupTable, fixedAccounts: fixedAddresses(fixed),
        selectedOutcome: outcome(selected), rawQuantity: rawU64(quantity),
      });
      setState({ kind: 'ready', inspection: next,
        message: `Terminal semantics joined at finalized slot ${next.observedSlot}: K=${next.representationWidth} claims over N=${next.resultOutcomeCount} results.` });
    } catch (error) { setState({ kind: 'refused', message: `Refused: ${errorMessage(error)}` }); }
  }

  return <>
    <section className="trade-v3-card">
      <header><span>07</span><div><h2>Read a real terminal payout without forging Custody authority</h2><p>The terminal winner belongs to Core. ProductBasisV3 may define K claim curves over a different N-way result partition. This browser rechecks that immutable semantic join and displays exact raw payout, including zero.</p></div></header>
      <div className="trade-v3-evidence"><article><span>Result width</span><strong>N</strong><small>Product-owned terminal partition</small></article><article><span>Claim width</span><strong>K</strong><small>representation basis; need not equal N</small></article><article><span>Losing claim</span><strong>zero is valid</strong><small>no fake one-atom Custody transfer</small></article><article><span>Submission</span><strong>Rust-emitter gated</strong><small>no parallel TypeScript digest authority</small></article></div>
    </section>

    <form className="trade-v3-card route-card" onSubmit={(event) => void inspect(event)}>
      <header><span>08</span><div><h2>Authenticate CapabilityV4 and evaluate Product terminal semantics</h2><p>The browser derives the selected basis record from fixed-frame content addressing, checks its Product/domain semantic identity, and follows Core&apos;s authenticated rational coordinate only when the resolved scenario requires one.</p></div></header>
      <div className="direct-form-grid"><label><span>Finalized RPC endpoint</span><input type="url" required value={endpoint} onChange={(event) => setEndpoint(event.target.value.trim())} /></label><label><span>Transaction payer identity</span><input required value={payer} onChange={(event) => setPayer(event.target.value.trim())} /></label><label><span>Representation actor</span><input required value={actor} onChange={(event) => setActor(event.target.value.trim())} /></label><label><span>Representation descriptor digest · 64 hex</span><input required value={descriptor} onChange={(event) => setDescriptor(event.target.value.trim().toLowerCase())} /></label><label><span>Selected native claim · zero based</span><input inputMode="numeric" required value={selected} onChange={(event) => setSelected(event.target.value.trim())} /></label><label><span>Raw native claim quantity</span><input inputMode="numeric" required value={quantity} onChange={(event) => setQuantity(event.target.value.trim())} /></label><label><span>Canonical address lookup table</span><input required value={lookupTable} onChange={(event) => setLookupTable(event.target.value.trim())} /></label></div>
      <label><span>Hot fixed38 addresses · one canonical base58 address per line</span><textarea required rows={12} value={fixed} onChange={(event) => setFixed(event.target.value)} /></label>
      <WalletDirectory directory={wallets} purpose="payer / actor identity" onConnected={adoptIdentity} />
      <div className="direct-actions"><button disabled={state.kind === 'loading'}>{state.kind === 'loading' ? 'Reading terminal semantics…' : 'Evaluate chain-derived terminal payout'}</button></div>
      <p className="direct-status">{walletStatus}</p><p className="direct-status" aria-live="polite">{state.message}</p>
      {inspection && summary && <div className="trade-v3-evidence"><article><span>Independent widths</span><strong>K={inspection.representationWidth} · N={inspection.resultOutcomeCount}</strong><small>winner {inspection.terminalWinner} · claim {inspection.selectedOutcome}</small></article><article><span>Raw burn</span><strong>{inspection.rawShardBurn.toString()} shard atoms</strong><small>{inspection.rawQuantity.toString()} native claim units</small></article><article><span>Exact payout</span><strong>{inspection.payout.rawPayout.toString()} collateral atoms</strong><small>{inspection.payout.losing ? 'valid losing zero path' : `${inspection.payout.payoutPerShard.toString()} per native claim`}</small></article><article><span>Scenario</span><strong>{inspection.payout.scenario}</strong><small>{summary.basis.slice(0, 16)}… ProductBasisV3</small></article></div>}
      {inspection && <div className="direct-output"><dl><div><dt>Execution status</dt><dd>{inspection.refusal}</dd></div><div><dt>Semantic projection</dt><dd>This page is an untrusted client projection. Onchain Claims repeats ProductBasis evaluation and the Rust operator alone emits SignedDeltaV3/Custody material.</dd></div></dl></div>}
    </form>
  </>;
}
