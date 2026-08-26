'use client';

import Link from 'next/link';
import { FormEvent, useState } from 'react';

import {
  type BearerTransferInspectionV2,
  type BearerTransferPlanV2,
  buildUnsignedBearerTransferV2,
  inspectBearerTransferV2,
  tokenBehaviorSummaryV2,
} from '@/lib/rationalTokenV2';
import { SolanaRpcClient } from '@/lib/rpc';
import {
  type WalletSignedTransactionV1,
  requestReadonlyWalletIdentityV1,
  requestWalletTransactionSignatureV1,
} from '@/lib/walletHandoff';
import RationalRetireReceiptPanel from './RationalRetireReceiptPanel';
import RationalOpenPanel from './RationalOpenPanel';
import RationalTerminalPanel from './RationalTerminalPanel';

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

export default function RationalRepresentationWorkspace() {
  const [endpoint, setEndpoint] = useState('http://127.0.0.1:8899');
  const [payer, setPayer] = useState('');
  const [authority, setAuthority] = useState('');
  const [coreProgram, setCoreProgram] = useState('');
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
  const [signed, setSigned] = useState<WalletSignedTransactionV1 | null>(null);
  const inspection = state.kind === 'ready' ? state.inspection : null;
  const summary = inspection === null ? null : tokenBehaviorSummaryV2(inspection);

  async function inspect(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setPlan(null); setSigned(null);
    setState({ kind: 'loading', message: 'Deriving the behavior record from finalized Market state and reacquiring the Mint, Token Accounts, and ALT…' });
    try {
      const next = await inspectBearerTransferV2(new SolanaRpcClient(endpoint), {
        payer, authority, coreProgram, market, mint, source, destination, lookupTable,
      });
      setState({
        kind: 'ready', inspection: next,
        message: `Exact TokenBehaviorSelectionV2 and extension-safe Token-2022 route joined at finalized slot ${next.observedSlot}.`,
      });
    } catch (error) {
      setState({ kind: 'refused', message: `Refused: ${errorMessage(error)}` });
    }
  }

  async function build() {
    if (inspection === null) return;
    setPlan(null); setSigned(null);
    try {
      const client = new SolanaRpcClient(endpoint);
      const blockhash = await client.latestBlockhash(inspection.observedSlot);
      const next = buildUnsignedBearerTransferV2(inspection, blockhash.blockhash, parseRawU64(rawAmount));
      setPlan(next);
      setBuildStatus(`Unsigned v0 TransferChecked packet: ${next.wireBytes.length} / 1232 bytes · ${next.loadedAddresses} ALT addresses · ${next.rawAmount.toString()} raw atoms.`);
    } catch (error) {
      setBuildStatus(`Refused: ${errorMessage(error)}`);
    }
  }

  async function connectWallet() {
    try {
      const identity = await requestReadonlyWalletIdentityV1(window.solana);
      setWallet(identity.address);
      if (payer === '') setPayer(identity.address);
      if (authority === '') setAuthority(identity.address);
      setWalletStatus(`${identity.address} · identity only; no signature requested`);
    } catch (error) {
      setWalletStatus(`Refused: ${errorMessage(error)}`);
    }
  }

  async function signTransaction() {
    if (plan === null) return;
    if (payer !== authority) {
      setWalletStatus('This packet has distinct payer and transfer-authority signers. Export it for explicit multisigner coordination.');
      return;
    }
    try {
      const next = await requestWalletTransactionSignatureV1(window.solana, plan.transaction, authority);
      setSigned(next);
      setWalletStatus(next.complete ? 'The sole required signature is complete. Nothing has been submitted.' : 'Wallet signed its authorized slot; more signatures remain.');
    } catch (error) {
      setWalletStatus(`Refused: ${errorMessage(error)}`);
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

  return <main className="product-shell trade-v3-shell">
    <header className="product-nav"><Link className="brand" href="/"><span className="brand-mark">dC</span><span>dClutch</span></Link><nav><Link href="/create">Create</Link><Link href="/trade">Trade</Link><Link href="/liquidity">Liquidity</Link><Link className="active" href="/redeem">Represent</Link><Link href="/release">Release</Link></nav><span className="preview-control"><i className="preview-dot" />raw-u64 economics</span></header>

    <section className="trade-v3-hero"><div><p className="eyebrow">Bearer · Rational · Structured successor boundary</p><h1>Decimals are a label.<br /><em>Atoms are the economics.</em></h1><p>The executable path below is an ordinary Token-2022 transfer selected by immutable Market Realm and release state. Rational open, terminal, and compact receipt retirement consume finalized CapabilityV4 and Product semantics without making static clients an authority.</p></div><aside><span>Executable now</span><strong>Bearer transfer</strong><p>Open and retirement expose bounded unsigned candidates. Terminal redemption is real and SBF-tested, while this browser remains read-only until it consumes the canonical Rust SignedDelta/Custody emitter.</p></aside></section>

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
      <header><span>03</span><div><h2>Wallet handoff and exact packet export</h2><p>Connecting reads identity only. Signing is a separate explicit action. Distinct payer/authority packets remain valid but are exported for multisigner coordination; nothing is submitted here.</p></div></header>
      <div className="signing-grid"><article><span>Wallet identity</span><strong>{wallet || 'not connected'}</strong><button type="button" onClick={() => void connectWallet()}>Connect identity</button><p>{walletStatus}</p></article><article><span>Unsigned / signed packet</span><strong>{plan ? `${plan.wireBytes.length} bytes · ${plan.loadedAddresses} ALT` : 'no packet built'}</strong><button type="button" disabled={plan === null} onClick={() => void signTransaction()}>Sign sole-wallet packet</button><button type="button" disabled={plan === null} onClick={downloadPacket}>Download exact packet</button><p>No automatic submission or hidden retry.</p></article></div>
      {plan && <details className="trade-v3-bytes"><summary>Exact transfer material</summary><dl><div><dt>Instruction bytes · base64</dt><dd>{base64(plan.instructionBytes)}</dd></div><div><dt>Packet bytes · base64</dt><dd>{base64(signed?.wireBytes ?? plan.wireBytes)}</dd></div></dl></details>}
    </section>

    <RationalOpenPanel />

    <RationalTerminalPanel />

    <RationalRetireReceiptPanel />

    <footer className="product-footer"><span>Arbitrary u8 display decimals · exact raw-u64 economics</span><span>No mock token state · no hidden rounding · no submit path</span></footer>
  </main>;
}
