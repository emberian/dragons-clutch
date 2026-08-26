'use client';

import Link from 'next/link';
import { FormEvent, useState } from 'react';

import {
  enumerateCoreMarketAddressesV1,
  inspectMarketDiscoveryV1,
  parseMarketAddressListV1,
  provenanceChipV1,
  shortAddressV1,
  type MarketCapabilityManifestV1,
  type MarketDiscoveryCardV1,
  type MarketDiscoveryV1,
  type MarketEnumerationV1,
} from '@/lib/marketDiscovery';
import { SolanaRpcClient, type ConnectionFacts } from '@/lib/rpc';

type State =
  | Readonly<{ kind: 'idle' | 'loading' | 'refused'; message: string }>
  | Readonly<{ kind: 'ready'; message: string; discovery: MarketDiscoveryV1; facts: ConnectionFacts }>;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'discovery refused without a usable reason';
}

function CapabilityBadges({ capabilities }: Readonly<{ capabilities: MarketCapabilityManifestV1 }>) {
  if (capabilities.status !== 'authenticated') {
    return <p className="market-capability-refusal">
      <span>{capabilities.status === 'unread' ? 'capabilities unread' : 'capabilities refused'}</span>
      {capabilities.reason}
    </p>;
  }
  return <div className="market-capability-row">
    {capabilities.badges.map((badge) => (
      <span key={badge.index} className={`capability-badge${badge.recognized ? ' recognized' : ''}`} title={`kind ${badge.kindId}`}>
        {badge.label}
        <small>{badge.activation === 'deadline' ? `deadline ${badge.deadline}` : 'immediate'}{badge.dependencies.length > 0 ? ` · after ${badge.dependencies.join(', ')}` : ''}</small>
      </span>
    ))}
  </div>;
}

function MarketCard({ card }: Readonly<{ card: MarketDiscoveryCardV1 }>) {
  if (card.status === 'refused') {
    return <article className="market-discovery-card refused">
      <div className="market-card-top"><span className="provenance-chip refused">{provenanceChipV1(card.provenance)}</span><span className="phase-chip">no phase</span></div>
      <h3 title={card.address}>{shortAddressV1(card.address, 10)}</h3>
      <p className="market-refusal">{card.refusal}</p>
      <p className="market-observation">Finalized observation slot {card.observedSlot}</p>
    </article>;
  }
  return <article className="market-discovery-card">
    <div className="market-card-top">
      <span className="provenance-chip">{provenanceChipV1(card.provenance)}</span>
      <span className={`phase-chip phase-${card.phase.toLowerCase()}`}>{card.phase}</span>
    </div>
    <h3 title={card.address}>{shortAddressV1(card.address, 10)}</h3>
    <dl className="market-card-facts">
      <div><dt>Hoard atoms · raw u64</dt><dd>{card.hoardAtoms}</dd></div>
      <div><dt>Generation</dt><dd>{card.generation}</dd></div>
      <div><dt>Outcome count</dt><dd>{card.outcomeCount}</dd></div>
      <div><dt>Outstanding children</dt><dd>{card.outstandingChildren}</dd></div>
      <div><dt>Per-outcome supply · raw u64</dt><dd>{card.supplyAtoms.join(' · ')}</dd></div>
      <div><dt>Settlement</dt><dd>{card.settlement.status === 'resolved'
        ? `${card.settlement.label} · state ${card.settlement.winner} · sequence ${card.settlement.terminalSequence}`
        : card.settlement.label}</dd></div>
      <div><dt>Collateral mint</dt><dd>{card.collateral.status === 'bound'
        ? <span title={card.collateral.collateralMint}>{card.collateral.collateralMintShort}</span>
        : 'unbound'}</dd></div>
      <div><dt>Realm content ID</dt><dd title={card.collateral.realmContentId}>{card.collateral.realmContentId.slice(0, 16)}…</dd></div>
      <div><dt>Finalized observed slot</dt><dd>{card.observedSlot}</dd></div>
    </dl>
    <p className="market-hoard-note">Hoard atoms are the Market&apos;s exact collateral principal. They are not liquidity, TVL, or a balance available to any participant.</p>
    {card.collateral.status === 'refused' && <p className="market-refusal">{card.collateral.reason}</p>}
    <CapabilityBadges capabilities={card.capabilities} />
    <ul className="market-bindings">
      {card.bindings.map((check) => (
        <li key={check.label} className={check.ok ? 'check-pass' : 'check-fail'}>
          <span aria-hidden="true">{check.ok ? '✓' : '×'}</span>
          <div><strong>{check.label}</strong><small>{check.detail}</small></div>
        </li>
      ))}
    </ul>
  </article>;
}

export default function MarketDiscoveryWorkspace() {
  const [endpoint, setEndpoint] = useState('http://127.0.0.1:8899');
  const [coreProgram, setCoreProgram] = useState('');
  const [registryProgram, setRegistryProgram] = useState('');
  const [addressList, setAddressList] = useState('');
  const [enumeration, setEnumeration] = useState<MarketEnumerationV1 | null>(null);
  const [enumerationStatus, setEnumerationStatus] = useState('No Core program enumeration has been attempted.');
  const [state, setState] = useState<State>({ kind: 'idle', message: 'No finalized Market discovery has been read.' });
  const discovery = state.kind === 'ready' ? state.discovery : null;

  async function enumerate() {
    setEnumeration(null);
    setEnumerationStatus('Attempting one bounded finalized getProgramAccounts scan of the selected Core program…');
    try {
      const next = await enumerateCoreMarketAddressesV1(new SolanaRpcClient(endpoint), coreProgram);
      setEnumeration(next);
      setEnumerationStatus(next.note);
      if (next.addresses.length > 0) setAddressList(next.addresses.join('\n'));
    } catch (error) {
      setEnumerationStatus(`Refused: ${errorMessage(error)}`);
    }
  }

  async function read(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setState({ kind: 'loading', message: 'Probing the endpoint, then reading every Market, its content-addressed Realm, and its capability manifest behind one finalized floor…' });
    try {
      const client = new SolanaRpcClient(endpoint);
      const facts = await client.probe();
      const addresses = parseMarketAddressListV1(addressList);
      const next = await inspectMarketDiscoveryV1(client, {
        coreProgramId: coreProgram,
        registryProgramId: registryProgram === '' ? null : registryProgram,
        addresses,
        enumeration: enumeration !== null && enumeration.mode === 'program-scan' && enumeration.addresses.join('\n') === addresses.join('\n') ? enumeration : undefined,
      });
      setState({ kind: 'ready', discovery: next, facts, message: next.reason });
    } catch (error) {
      setState({ kind: 'refused', message: `Refused: ${errorMessage(error)}` });
    }
  }

  return <main className="product-shell trade-v3-shell">
    <header className="product-nav">
      <Link className="brand" href="/"><span className="brand-mark">dC</span><span>dClutch</span></Link>
      <nav>
        <Link className="active" href="/markets">Markets</Link>
        <Link href="/create">Create</Link>
        <Link href="/trade">Trade</Link>
        <Link href="/liquidity">Liquidity</Link>
        <Link href="/redeem">Represent</Link>
        <Link href="/release">Release</Link>
      </nav>
      <span className="preview-control"><i className="preview-dot" />raw-u64 economics</span>
    </header>

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">Market discovery · finalized reads only</p>
        <h1>Every card is a read.<br /><em>Or it says REFUSED.</em></h1>
        <p>Discovery lists exactly what finalized Core state justifies: phase, generation, outcome width, exact Hoard atoms, settlement truth, the content-addressed Realm behind the collateral mint, and the capability manifest a Market actually authenticated. There is no volume, price, odds, probability, or yield here, because none of those are facts this chain persists.</p>
      </div>
      <aside>
        <span>Provenance</span>
        <strong>CHAIN · finalized slot</strong>
        <p>Each surface carries its own provenance chip. A surface that cannot be decoded or bound carries REFUSED and its exact reason instead of a blank or a zero.</p>
      </aside>
    </section>

    <section className="trade-v3-card">
      <header><span>00</span><div><h2>What a discovery card is allowed to claim</h2><p>The browser is an untrusted projection. It decodes Core accounts hostilely, derives the Realm the Market itself commits to, and authenticates the capability manifest against its content identity before any badge appears.</p></div></header>
      <div className="trade-v3-evidence">
        <article><span>Economics</span><strong>raw u64 atoms</strong><small>Hoard principal is never liquidity or TVL</small></article>
        <article><span>Discovery</span><strong>bounded</strong><small>known addresses or one finalized program scan</small></article>
        <article><span>Capabilities</span><strong>manifest-only</strong><small>no capability is asserted from the root alone</small></article>
        <article><span>Refusal</span><strong>explicit</strong><small>every undecoded surface names its reason</small></article>
      </div>
    </section>

    <form className="trade-v3-card route-card" onSubmit={(event) => void read(event)}>
      <header><span>01</span><div><h2>Select one finalized endpoint and Core authority</h2><p>dClutch publishes no index. Markets are enumerated from addresses you already know, or from one bounded finalized getProgramAccounts scan when the endpoint serves it. A Registry program is optional and is required only to authenticate capability manifests.</p></div></header>
      <div className="direct-form-grid">
        <label><span>Finalized RPC endpoint</span><input type="url" required value={endpoint} onChange={(event) => setEndpoint(event.target.value.trim())} /></label>
        <label><span>Core program</span><input required value={coreProgram} onChange={(event) => setCoreProgram(event.target.value.trim())} /></label>
        <label><span>Registry program · optional</span><input value={registryProgram} onChange={(event) => setRegistryProgram(event.target.value.trim())} /></label>
      </div>
      <label><span>Known Market addresses · one canonical base58 address per line</span><textarea rows={6} value={addressList} onChange={(event) => setAddressList(event.target.value)} /></label>
      <div className="direct-actions">
        <button type="button" onClick={() => void enumerate()}>Enumerate Markets from the Core program</button>
        <button disabled={state.kind === 'loading'}>{state.kind === 'loading' ? 'Reading finalized Market state…' : 'Read finalized Market discovery'}</button>
      </div>
      <p className="direct-status">{enumerationStatus}</p>
      <p className="direct-status" aria-live="polite">{state.message}</p>
    </form>

    <section className="trade-v3-card">
      <header><span>02</span><div><h2>Discovered Markets</h2><p>One card per requested address. A card is decoded or refused; it is never partially invented.</p></div></header>
      {discovery === null && <p className="market-empty">No finalized listing has been read. A local validator at <code>http://127.0.0.1:8899</code> is the expected first endpoint; until one answers, this surface stays empty rather than showing placeholder Markets.</p>}
      {discovery !== null && state.kind === 'ready' && <>
        <div className="trade-v3-evidence">
          <article><span>Endpoint</span><strong>{state.facts.solanaCore}</strong><small>genesis {shortAddressV1(state.facts.genesisHash, 6)}</small></article>
          <article><span>Finalized floor</span><strong>{discovery.floorSlot}</strong><small>one observation epoch for every card</small></article>
          <article><span>Enumeration</span><strong>{discovery.enumeration.mode}</strong><small>{discovery.enumeration.addresses.length} address{discovery.enumeration.addresses.length === 1 ? '' : 'es'}</small></article>
          <article><span>Capability source</span><strong>{discovery.registryProgramId === null ? 'not selected' : shortAddressV1(discovery.registryProgramId, 6)}</strong><small>{discovery.registryProgramId === null ? 'manifests unread' : 'manifests authenticated by content'}</small></article>
        </div>
        <p className="direct-status">{discovery.enumeration.note}</p>
        {discovery.cards.length === 0
          ? <p className="market-empty">This finalized floor holds no requested Market. Nothing is displayed in place of one.</p>
          : <div className="market-card-grid">{discovery.cards.map((card) => <MarketCard key={card.address} card={card} />)}</div>}
      </>}
    </section>

    <footer className="product-footer">
      <span>Chain-derived phase, atoms, and refusals only</span>
      <span>No volume · no odds · no probability · no yield</span>
    </footer>
  </main>;
}
