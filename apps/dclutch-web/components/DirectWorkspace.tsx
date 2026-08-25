'use client';

import {
  AddressLookupTableAccount,
  AddressLookupTableProgram,
  PublicKey,
} from '@solana/web3.js';
import Link from 'next/link';
import { FormEvent, useMemo, useState } from 'react';

import type { DecodedProjection } from '@/lib/decoders';
import { inspectDirectFeePolicy, type DirectFeePolicyObservation } from '@/lib/directChain';
import { decodeCompactIntentV1, type CompactIntentV1 } from '@/lib/directCodec';
import {
  CLAIM_PROGRAM_ID,
  CUSTODY_PROGRAM_ID,
  buildUnsignedDirectTransaction,
  deriveDirectAddresses,
  encodeIntentSigningPayload,
} from '@/lib/directTransaction';
import { scanProgram, SolanaRpcClient, type ConnectionFacts, type ProgramSnapshot } from '@/lib/rpc';

type Ready = Readonly<{ facts: ConnectionFacts; snapshot: ProgramSnapshot }>;
type Status = Readonly<{ kind: 'idle' | 'loading' | 'error'; message?: string }> | Readonly<{ kind: 'ready'; value: Ready }>;
type BuildOutput = Readonly<{ intentHex: string; intentBase64: string; policy: DirectFeePolicyObservation }>;
type TransactionOutput = Readonly<{ base64: string; wireBytes: number; lookupAddresses: number; blockhashSlot: string; lastValidBlockHeight: string }>;

type MatchEnvelope = Readonly<{
  payer: string;
  lookupTable: string;
  fill: string;
  executionPrice: string;
  seller: Readonly<{ maker: string; signatureHex: string; intentBase64: string }>;
  buyer: Readonly<{ maker: string; signatureHex: string; intentBase64: string }>;
  routing: Readonly<{
    journal: string; realm: string; feePolicy: string; capabilityManifest: string; mint: string;
    buyerSource: string; sellerDestination: string; feeDestination: string; tokenProgram: string;
  }>;
}>;

function message(error: unknown): string {
  return error instanceof Error ? error.message : 'The operation failed without a usable error message.';
}

function unsigned(value: string, field: string): bigint {
  if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new Error(`${field} must be a canonical unsigned integer`);
  const parsed = BigInt(value);
  if (parsed > 18_446_744_073_709_551_615n) throw new Error(`${field} exceeds u64`);
  return parsed;
}

function exactBase64(value: string, width: number, field: string): Uint8Array {
  if (value.trim() !== value || value.length === 0) throw new Error(`${field} is not canonical base64 text`);
  let binary: string;
  try { binary = atob(value); } catch { throw new Error(`${field} is not valid base64`); }
  const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
  if (bytes.length !== width) throw new Error(`${field} must decode to exactly ${width} bytes`);
  return bytes;
}

function signature(value: string, field: string): Uint8Array {
  if (!/^[0-9a-f]{128}$/.test(value)) throw new Error(`${field} must be exactly 64 lowercase-hex bytes`);
  return Uint8Array.from(value.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16));
}

function transactionBase64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

function openMarkets(snapshot: ProgramSnapshot): DecodedProjection[] {
  return snapshot.projections.filter((projection): projection is DecodedProjection =>
    projection.status === 'decoded'
      && projection.semantics.kind === 'Market'
      && projection.semantics.phase === 'Open'
      && projection.bindings.length > 0
      && projection.bindings.every((check) => check.ok));
}

function placeholderEnvelope(): string {
  return JSON.stringify({
    payer: '', lookupTable: '', fill: '0', executionPrice: '0',
    seller: { maker: '', signatureHex: '', intentBase64: '' },
    buyer: { maker: '', signatureHex: '', intentBase64: '' },
    routing: { journal: '', realm: '', feePolicy: '', capabilityManifest: '', mint: '', buyerSource: '', sellerDestination: '', feeDestination: '', tokenProgram: '' },
  }, null, 2);
}

export default function DirectWorkspace() {
  const [endpoint, setEndpoint] = useState('http://127.0.0.1:8899');
  const [protocolProgram, setProtocolProgram] = useState('');
  const [controllerProgram, setControllerProgram] = useState('');
  const [status, setStatus] = useState<Status>({ kind: 'idle' });
  const [marketAddress, setMarketAddress] = useState('');
  const [maker, setMaker] = useState('');
  const [collateral, setCollateral] = useState('');
  const [feePolicy, setFeePolicy] = useState('');
  const [side, setSide] = useState('0');
  const [outcome, setOutcome] = useState('0');
  const [lifecycle, setLifecycle] = useState('0');
  const [nonce, setNonce] = useState('0');
  const [validFrom, setValidFrom] = useState('0');
  const [validThrough, setValidThrough] = useState('18446744073709551615');
  const [maximumFill, setMaximumFill] = useState('0');
  const [limitPrice, setLimitPrice] = useState('0');
  const [intentStatus, setIntentStatus] = useState('');
  const [intentOutput, setIntentOutput] = useState<BuildOutput | null>(null);
  const [envelope, setEnvelope] = useState(placeholderEnvelope);
  const [transactionStatus, setTransactionStatus] = useState('');
  const [transaction, setTransaction] = useState<TransactionOutput | null>(null);

  const ready = status.kind === 'ready' ? status.value : null;
  const markets = useMemo(() => ready === null ? [] : openMarkets(ready.snapshot), [ready]);
  const selected = markets.find((market) => market.address === marketAddress) ?? null;

  async function connect(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setStatus({ kind: 'loading', message: 'Reading finalized program state…' });
    setIntentOutput(null);
    setTransaction(null);
    try {
      const client = new SolanaRpcClient(endpoint);
      const [facts, snapshot] = await Promise.all([client.probe(), scanProgram(client, protocolProgram)]);
      const value = Object.freeze({ facts, snapshot });
      const observedMarkets = openMarkets(snapshot);
      setMarketAddress(observedMarkets[0]?.address ?? '');
      setStatus({ kind: 'ready', value });
    } catch (error) {
      setStatus({ kind: 'error', message: message(error) });
    }
  }

  async function buildIntent(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIntentOutput(null);
    if (ready === null || selected === null || selected.semantics.kind !== 'Market') return;
    setIntentStatus('Authenticating the immutable fee policy at the finalized Market floor…');
    try {
      const policy = await inspectDirectFeePolicy(new SolanaRpcClient(endpoint), protocolProgram, feePolicy, ready.snapshot.scanSlot);
      const outcomeIndex = Number(unsigned(outcome, 'outcome'));
      if (outcomeIndex >= selected.semantics.outcomeCount) throw new Error(`outcome must be below the chain-derived width ${selected.semantics.outcomeCount}`);
      const sideTag = Number(unsigned(side, 'side'));
      const lifecycleTag = Number(unsigned(lifecycle, 'lifecycle'));
      if (sideTag > 1) throw new Error('side must be seller 0 or buyer 1');
      if (lifecycleTag > 1) throw new Error('compiled lifecycle must be FOK 0 or IOC 1');
      const intent: CompactIntentV1 = {
        side: sideTag,
        outcome: outcomeIndex,
        lifecycle: lifecycleTag,
        market: new PublicKey(selected.address).toBytes(),
        generation: BigInt(selected.semantics.generation),
        nonce: unsigned(nonce, 'nonce'),
        validFrom: unsigned(validFrom, 'valid-from slot'),
        validThrough: unsigned(validThrough, 'valid-through slot'),
        maximumFill: unsigned(maximumFill, 'maximum fill'),
        limitPrice: unsigned(limitPrice, 'limit price'),
        feeBasisPoints: policy.feeBasisPoints,
        collateralAccount: new PublicKey(collateral).toBytes(),
      };
      new PublicKey(maker);
      const encoded = encodeIntentSigningPayload(intent);
      setIntentOutput({ intentHex: encoded.hex, intentBase64: encoded.base64, policy });
      setIntentStatus('Exact 136-byte signing payload constructed. No signature was requested or produced.');
    } catch (error) {
      setIntentStatus(`Refused: ${message(error)}`);
    }
  }

  async function buildTransaction(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setTransaction(null);
    if (ready === null || selected === null) return;
    setTransactionStatus('Acquiring the lookup table, routing accounts, and a finalized blockhash…');
    try {
      const parsed = JSON.parse(envelope) as MatchEnvelope;
      const sellerIntent = decodeCompactIntentV1(exactBase64(parsed.seller.intentBase64, 136, 'seller intent'));
      const buyerIntent = decodeCompactIntentV1(exactBase64(parsed.buyer.intentBase64, 136, 'buyer intent'));
      if (!new PublicKey(sellerIntent.market).equals(new PublicKey(selected.address)) || !new PublicKey(buyerIntent.market).equals(new PublicKey(selected.address))) throw new Error('match intents do not select the observed open Market');
      const client = new SolanaRpcClient(endpoint);
      const blockhash = await client.latestBlockhash(ready.snapshot.scanSlot);
      const lookupObservation = await client.accountInfo(parsed.lookupTable, ready.snapshot.scanSlot);
      if (lookupObservation.account === null || lookupObservation.account.owner !== AddressLookupTableProgram.programId.toBase58() || lookupObservation.account.executable) throw new Error('lookup table is absent or is not an Address Lookup Table account');
      const lookupTable = new AddressLookupTableAccount({ key: new PublicKey(parsed.lookupTable), state: AddressLookupTableAccount.deserialize(lookupObservation.account.data) });
      const derived = deriveDirectAddresses(controllerProgram, selected.address, parsed.seller.maker, parsed.buyer.maker, sellerIntent.generation, sellerIntent.outcome, buyerIntent.outcome);
      const addresses = [
        controllerProgram, derived.controller.toBase58(), derived.sellerReplay.toBase58(), derived.buyerReplay.toBase58(),
        derived.sellerPosition.toBase58(), derived.buyerPosition.toBase58(), CLAIM_PROGRAM_ID.toBase58(), CUSTODY_PROGRAM_ID.toBase58(),
        selected.address, parsed.routing.journal, parsed.routing.realm, parsed.routing.feePolicy, parsed.routing.capabilityManifest,
        parsed.routing.mint, parsed.routing.buyerSource, parsed.routing.sellerDestination, parsed.routing.feeDestination, parsed.routing.tokenProgram,
      ];
      if (new Set(addresses).size !== addresses.length) throw new Error('routing aliases two roles that must remain distinct');
      const observations = await Promise.all(addresses.map((address) => client.accountInfo(address, ready.snapshot.scanSlot)));
      if (observations.some((observation) => observation.account === null)) throw new Error('one or more exact Direct routing accounts are absent at the finalized floor');
      const accounts = observations.map((observation) => observation.account!);
      if (!accounts[0].executable || accounts[8].owner !== protocolProgram || accounts[9].owner !== controllerProgram
          || !accounts[6].executable || !accounts[7].executable || !accounts[17].executable
          || accounts.slice(13, 17).some((account) => account.owner !== parsed.routing.tokenProgram)) {
        throw new Error('finalized routing account owners/executable flags do not match the compiled Direct roles');
      }
      const report = buildUnsignedDirectTransaction({
        controllerProgram, market: selected.address, payer: parsed.payer, recentBlockhash: blockhash.blockhash,
        fill: unsigned(parsed.fill, 'fill'), executionPrice: unsigned(parsed.executionPrice, 'execution price'),
        seller: { maker: parsed.seller.maker, signature: signature(parsed.seller.signatureHex, 'seller signature'), intent: sellerIntent },
        buyer: { maker: parsed.buyer.maker, signature: signature(parsed.buyer.signatureHex, 'buyer signature'), intent: buyerIntent },
        routing: parsed.routing, lookupTable,
      });
      setTransaction({ base64: transactionBase64(report.wireBytes), wireBytes: report.wireBytes.length, lookupAddresses: report.lookupAddressesUsed, blockhashSlot: blockhash.slot, lastValidBlockHeight: blockhash.lastValidBlockHeight });
      setTransactionStatus('Unsigned transaction built. It has not been signed by the payer or submitted.');
    } catch (error) {
      setTransactionStatus(`Refused: ${message(error)}`);
    }
  }

  return (
    <main className="product-shell direct-workspace">
      <header className="product-nav"><Link className="brand" href="/"><span className="brand-mark">dC</span><span>dClutch</span></Link><nav><Link href="/">Markets</Link><Link className="active" href="/direct">Direct</Link><Link href="/explorer">Explorer</Link></nav><div className="preview-control"><span className="preview-dot" /> Local / user-selected RPC</div></header>
      <section className="market-heading"><div><div className="market-kicker"><span>Real chain state</span><span>Unsigned construction</span></div><h1>Build what the controller will actually read.</h1><p>No sample market, price, balance, signature, or route is supplied. This workspace derives an open Market from a finalized scan, authenticates its immutable fee record, and constructs the exact compiled Direct bytes.</p></div></section>

      <form className="direct-card" onSubmit={connect}>
        <div className="direct-card-heading"><span>01</span><div><h2>Acquire protocol state</h2><p>Local validator by default; no request occurs before submit.</p></div></div>
        <div className="direct-form-grid"><label><span>RPC endpoint</span><input type="url" required value={endpoint} onChange={(event) => setEndpoint(event.target.value)} /></label><label><span>Protocol program</span><input required value={protocolProgram} onChange={(event) => setProtocolProgram(event.target.value.trim())} /></label><label><span>Compiled Direct controller</span><input required value={controllerProgram} onChange={(event) => setControllerProgram(event.target.value.trim())} /></label></div>
        <button type="submit" disabled={status.kind === 'loading'}>{status.kind === 'loading' ? 'Reading finalized state…' : 'Connect & discover Markets'}</button>
        <p className="direct-status">{status.kind === 'error' ? `Refused: ${status.message}` : status.kind === 'ready' ? `${markets.length} canonical open Market${markets.length === 1 ? '' : 's'} at slot ${status.value.snapshot.scanSlot} · ${status.value.facts.solanaCore}` : status.message ?? 'Waiting for an explicit connection.'}</p>
      </form>

      {ready !== null && <form className="direct-card" onSubmit={buildIntent}>
        <div className="direct-card-heading"><span>02</span><div><h2>Construct one maker intent</h2><p>The output is a signing payload, not a transaction or an order-book claim.</p></div></div>
        {markets.length === 0 ? <p className="direct-refusal">No decoded, binding-clean Open Market exists in this scan. Nothing can be fabricated.</p> : <>
          <div className="direct-form-grid"><label><span>Observed open Market</span><select value={marketAddress} onChange={(event) => setMarketAddress(event.target.value)}>{markets.map((market) => <option value={market.address} key={market.address}>{market.address}</option>)}</select></label><label><span>Maker Ed25519 public key</span><input required value={maker} onChange={(event) => setMaker(event.target.value.trim())} /></label><label><span>Collateral token account</span><input required value={collateral} onChange={(event) => setCollateral(event.target.value.trim())} /></label><label><span>Canonical fee-policy record</span><input required value={feePolicy} onChange={(event) => setFeePolicy(event.target.value.trim())} /></label><label><span>Role</span><select value={side} onChange={(event) => setSide(event.target.value)}><option value="0">Seller</option><option value="1">Buyer</option></select></label><label><span>Lifecycle</span><select value={lifecycle} onChange={(event) => setLifecycle(event.target.value)}><option value="0">Fill or kill</option><option value="1">Immediate or cancel</option></select></label><label><span>Outcome index</span><input inputMode="numeric" value={outcome} onChange={(event) => setOutcome(event.target.value)} /></label><label><span>Gap-free nonce</span><input inputMode="numeric" value={nonce} onChange={(event) => setNonce(event.target.value)} /></label><label><span>Valid from slot</span><input inputMode="numeric" value={validFrom} onChange={(event) => setValidFrom(event.target.value)} /></label><label><span>Valid through slot</span><input inputMode="numeric" value={validThrough} onChange={(event) => setValidThrough(event.target.value)} /></label><label><span>Maximum fill atoms</span><input inputMode="numeric" value={maximumFill} onChange={(event) => setMaximumFill(event.target.value)} /></label><label><span>Limit price · 1e6 scale</span><input inputMode="numeric" value={limitPrice} onChange={(event) => setLimitPrice(event.target.value)} /></label></div>
          <button type="submit">Authenticate policy & build payload</button><p className="direct-status">{intentStatus}</p>
          {intentOutput && <div className="direct-output"><dl><div><dt>Fee policy</dt><dd>{intentOutput.policy.feeBasisPoints} bps → {intentOutput.policy.recipient}</dd></div><div><dt>Policy digest</dt><dd>{intentOutput.policy.contentDigest}</dd></div></dl><label><span>136-byte payload · base64</span><textarea readOnly value={intentOutput.intentBase64} /></label><label><span>Exact bytes · lowercase hex</span><textarea readOnly value={intentOutput.intentHex} /></label></div>}
        </>}
      </form>}

      {ready !== null && markets.length > 0 && <form className="direct-card" onSubmit={buildTransaction}>
        <div className="direct-card-heading"><span>03</span><div><h2>Assemble a signed match</h2><p>Paste two externally signed exact intents and their real route. The browser reacquires every named account and a finalized blockhash, then emits an unsigned payer transaction.</p></div></div>
        <label><span>Match envelope · canonical field values, no private keys</span><textarea className="match-envelope" value={envelope} onChange={(event) => setEnvelope(event.target.value)} spellCheck={false} /></label>
        <button type="submit">Acquire route & build unsigned v0 transaction</button><p className="direct-status">{transactionStatus}</p>
        {transaction && <div className="direct-output"><dl><div><dt>Wire profile</dt><dd>{transaction.wireBytes} / 1232 bytes · {transaction.lookupAddresses} lookup addresses</dd></div><div><dt>Blockhash lifetime</dt><dd>slot {transaction.blockhashSlot} · valid through height {transaction.lastValidBlockHeight}</dd></div></dl><label><span>Unsigned v0 transaction · base64</span><textarea readOnly value={transaction.base64} /></label><p className="direct-refusal">Zero signature slots remain in this artifact. This page never signs or submits it.</p></div>}
      </form>}
      <footer className="product-footer"><span>Static clients are untrusted projections.</span><span>No wallet access · no signing · no submission</span></footer>
    </main>
  );
}
