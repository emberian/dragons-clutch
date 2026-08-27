'use client';

import Link from 'next/link';
import { FormEvent, useState } from 'react';

import WalletDirectory, { useWalletDirectoryV1 } from '@/components/WalletDirectory';
import {
  enumerateCoreMarketAddressesV1,
  parseMarketAddressListV1,
  provenanceChipV1,
  shortAddressV1,
  type MarketDiscoveryCardV1,
} from '@/lib/marketDiscovery';
import {
  inspectPortfolioV1,
  parsePortfolioOwnerV1,
  PORTFOLIO_MAX_MARKETS,
  type PortfolioEntryV1,
  type PortfolioV1,
} from '@/lib/portfolio';
import { SolanaRpcClient, type ConnectionFacts } from '@/lib/rpc';

type State =
  | Readonly<{ kind: 'idle' | 'loading' | 'refused'; message: string }>
  | Readonly<{ kind: 'ready'; message: string; portfolio: PortfolioV1; facts: ConnectionFacts }>;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'the portfolio read refused without a usable reason';
}

function MarketHeading({ market, address }: Readonly<{ market: MarketDiscoveryCardV1; address: string }>) {
  return <div className="market-card-top">
    <Link href={`/markets/${address}`} title={address}>{shortAddressV1(address, 8)}</Link>
    <span className={`provenance-chip${market.provenance.kind === 'refused' ? ' refused' : ''}`}>{provenanceChipV1(market.provenance)}</span>
    <span className={`phase-chip${market.status === 'decoded' ? ` phase-${market.phase.toLowerCase()}` : ''}`}>{market.status === 'decoded' ? market.phase : 'no phase'}</span>
  </div>;
}

function PositionEntry({ entry }: Readonly<{ entry: PortfolioEntryV1 }>) {
  const { position, market } = entry;
  return <article className={`portfolio-entry${position.status === 'refused' ? ' refused' : ''}`}>
    <MarketHeading market={market} address={entry.marketAddress} />
    <dl className="market-card-facts">
      <div><dt>Derived Claims aggregate</dt><dd title={entry.aggregateAddress ?? undefined}>{entry.aggregateAddress === null ? 'not derived' : shortAddressV1(entry.aggregateAddress, 8)}</dd></div>
      <div><dt>Derived Position address</dt><dd title={entry.positionAddress ?? undefined}>{entry.positionAddress === null ? 'not derived' : shortAddressV1(entry.positionAddress, 8)}</dd></div>
      <div><dt>Finalized observed slot</dt><dd>{position.observedSlot}</dd></div>
      {market.status === 'decoded' && <div><dt>Market generation</dt><dd>{market.generation}</dd></div>}
      {market.status === 'decoded' && market.collateral.status === 'bound' && <div><dt>Collateral mint</dt><dd title={market.collateral.collateralMint}>{market.collateral.collateralMintShort}</dd></div>}
    </dl>
    <div className="portfolio-position-provenance">
      <span className={`provenance-chip${position.provenance.kind === 'refused' ? ' refused' : ''}`}>{provenanceChipV1(position.provenance)}</span>
      <small>Position</small>
    </div>
    {position.status === 'absent' && <p className="market-empty">{position.note}</p>}
    {position.status === 'refused' && <p className="market-refusal">{position.reason}</p>}
    {position.status === 'held' && <>
      <h4 className="detail-subhead">Owned claim balances · ordered, raw u64</h4>
      <ol className="outcome-vector">
        {position.balances.map((amount, index) => (
          <li key={index} className={position.claim.kind === 'redeemable' && position.claim.winningClaim === index ? 'winning-outcome' : ''}>
            <span>claim {index}</span>
            <strong>{amount}</strong>
            {position.claim.kind === 'redeemable' && <small>{position.claim.winningClaim === index ? `redeems ${position.claim.redeemableAtoms} collateral atoms` : 'redeems 0 collateral atoms'}</small>}
          </li>
        ))}
      </ol>
      {position.claim.kind === 'mergeable' && <div className="portfolio-claim">
        <span>Complete sets mergeable</span>
        <strong>{position.claim.completeSetsAtoms}</strong>
        <p>{position.claim.note}</p>
      </div>}
      {position.claim.kind === 'redeemable' && <div className="portfolio-claim">
        <span>Redeemable collateral atoms</span>
        <strong>{position.claim.redeemableAtoms}</strong>
        <p>{position.claim.note}</p>
      </div>}
      {position.claim.kind === 'unavailable' && <p className="market-capability-refusal"><span>no transition available</span>{position.claim.note}</p>}
      <dl className="market-card-facts">
        <div><dt>Claims aggregate named by the Position</dt><dd title={position.aggregateAddress}>{shortAddressV1(position.aggregateAddress, 8)}</dd></div>
        <div><dt>Position revision</dt><dd>{position.revision}</dd></div>
        <div><dt>Claim width</dt><dd>{position.claimCount}</dd></div>
        <div><dt>Liability basis</dt><dd title={position.liabilityBasisId}>{position.liabilityBasisId.slice(0, 16)}…</dd></div>
      </dl>
    </>}
  </article>;
}

export default function PortfolioWorkspace() {
  const directory = useWalletDirectoryV1();
  const [endpoint, setEndpoint] = useState('http://127.0.0.1:8899');
  const [coreProgram, setCoreProgram] = useState('');
  const [claimsProgram, setClaimsProgram] = useState('');
  const [registryProgram, setRegistryProgram] = useState('');
  const [owner, setOwner] = useState('');
  const [addressList, setAddressList] = useState('');
  const [addOne, setAddOne] = useState('');
  const [enumerationStatus, setEnumerationStatus] = useState('No Core program enumeration has been attempted.');
  const [state, setState] = useState<State>({ kind: 'idle', message: 'No finalized Position state has been read.' });
  const portfolio = state.kind === 'ready' ? state.portfolio : null;

  function append(address: string) {
    const candidate = address.trim();
    if (candidate === '') return;
    setAddressList((current) => {
      const existing = current.split(/[\s,]+/).filter((entry) => entry.length > 0);
      return existing.includes(candidate) ? current : [...existing, candidate].join('\n');
    });
    setAddOne('');
  }

  async function enumerate() {
    setEnumerationStatus('Attempting one bounded finalized getProgramAccounts scan of the selected Core program…');
    try {
      const next = await enumerateCoreMarketAddressesV1(new SolanaRpcClient(endpoint), coreProgram);
      setEnumerationStatus(next.note);
      if (next.addresses.length > 0) setAddressList(next.addresses.join('\n'));
    } catch (error) {
      setEnumerationStatus(`Refused: ${errorMessage(error)}`);
    }
  }

  async function read(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setState({ kind: 'loading', message: 'Deriving one Claims aggregate and one Position address per named Market, then reading every derived address behind one finalized floor…' });
    try {
      const client = new SolanaRpcClient(endpoint);
      const facts = await client.probe();
      const next = await inspectPortfolioV1(client, {
        coreProgramId: coreProgram,
        claimsProgramId: claimsProgram === '' ? null : claimsProgram,
        registryProgramId: registryProgram === '' ? null : registryProgram,
        owner: parsePortfolioOwnerV1(owner),
        marketAddresses: parseMarketAddressListV1(addressList),
      });
      setState({ kind: 'ready', portfolio: next, facts, message: next.reason });
    } catch (error) {
      setState({ kind: 'refused', message: `Refused: ${errorMessage(error)}` });
    }
  }

  return <main className="product-shell trade-v3-shell">
    <header className="product-nav">
      <Link className="brand" href="/"><span className="brand-mark">dC</span><span>dClutch</span></Link>
      <nav>
        <Link href="/markets">Markets</Link>
        <Link className="active" href="/portfolio">Portfolio</Link>
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
        <p className="eyebrow">Portfolio · derived addresses, no index</p>
        <h1>Your claims, derived.<br /><em>Never looked up.</em></h1>
        <p>dClutch runs no indexer and this browser will not pretend to be one. A Position lives at the program-derived address of the Position seed domain plus the exact Market and owner keys, so an owner plus a Market address is enough to ask the chain directly. A derived address that holds no account is reported as exactly that, which is the honest chain state.</p>
      </div>
      <aside>
        <span>The honest gap</span>
        <strong>Markets you name</strong>
        <p>Positions are derived, but Markets are not: this surface can only read Markets you already know or that one bounded finalized program scan returns. Finding every Market an owner ever touched needs an index dClutch does not publish.</p>
      </aside>
    </section>

    <form className="trade-v3-card route-card" onSubmit={(event) => void read(event)}>
      <header><span>01</span><div><h2>Endpoint, Core authority, owner identity, and Markets</h2><p>A browser wallet is optional here. Connecting one reads a public address and nothing else; pasting any address reads the same finalized state for that owner, because reading a derived address requires no authority at all.</p></div></header>
      <div className="direct-form-grid">
        <label><span>Finalized RPC endpoint</span><input type="url" required value={endpoint} onChange={(event) => setEndpoint(event.target.value.trim())} /></label>
        <label><span>Core program</span><input required value={coreProgram} onChange={(event) => setCoreProgram(event.target.value.trim())} /></label>
        <label><span>Claims program</span><input required value={claimsProgram} onChange={(event) => setClaimsProgram(event.target.value.trim())} /></label>
        <label><span>Registry program · optional</span><input value={registryProgram} onChange={(event) => setRegistryProgram(event.target.value.trim())} /></label>
        <label><span>Owner address · wallet or pasted</span><input required value={owner} onChange={(event) => setOwner(event.target.value.trim())} /></label>
      </div>
      <WalletDirectory directory={directory} purpose="read one owner identity" onConnected={(address) => setOwner(address)} />

      <h3 className="detail-subhead">Markets to derive against</h3>
      <p className="direct-status">One canonical base58 Market address per line, up to the explicit {PORTFOLIO_MAX_MARKETS}-Market browser bound. Enumeration uses the same bounded finalized program scan the discovery surface uses; an endpoint that refuses that scan says so rather than returning an empty list.</p>
      <label><span>Known Market addresses</span><textarea rows={6} value={addressList} onChange={(event) => setAddressList(event.target.value)} /></label>
      <div className="direct-form-grid">
        <label><span>Add one Market address</span><input value={addOne} onChange={(event) => setAddOne(event.target.value.trim())} /></label>
      </div>
      <div className="direct-actions">
        <button type="button" className="secondary-action" onClick={() => append(addOne)}>Add this Market</button>
        <button type="button" onClick={() => void enumerate()}>Enumerate Markets from the Core program</button>
        <button disabled={state.kind === 'loading'}>{state.kind === 'loading' ? 'Reading finalized Position state…' : 'Derive and read Positions'}</button>
      </div>
      <p className="direct-status">{enumerationStatus}</p>
      <p className="direct-status" aria-live="polite">{state.message}</p>
    </form>

    <section className="trade-v3-card">
      <header><span>02</span><div><h2>Derived Positions</h2><p>One entry per named Market. Every entry states the address that was derived, whether an account exists there, and what the balances found admit under the Market&apos;s own phase and settlement.</p></div></header>
      {portfolio === null && <p className="market-empty">No finalized Position state has been read. Until an owner and at least one Market address are supplied and an endpoint answers, this surface stays empty rather than showing placeholder holdings.</p>}
      {portfolio !== null && state.kind === 'ready' && <>
        <div className="trade-v3-evidence">
          <article><span>Owner</span><strong>{shortAddressV1(portfolio.owner, 6)}</strong><small>identity only; nothing is signed here</small></article>
          <article><span>Finalized floor</span><strong>{portfolio.floorSlot}</strong><small>one observation epoch for every entry</small></article>
          <article><span>Endpoint</span><strong>{state.facts.solanaCore}</strong><small>genesis {shortAddressV1(state.facts.genesisHash, 6)}</small></article>
          <article><span>Derived addresses</span><strong>{portfolio.entries.length}</strong><small>one per named Market</small></article>
        </div>
        {portfolio.entries.length === 0
          ? <p className="market-empty">{portfolio.reason}</p>
          : <div className="market-card-grid">{portfolio.entries.map((entry) => <PositionEntry key={entry.marketAddress} entry={entry} />)}</div>}
      </>}
    </section>

    <footer className="product-footer">
      <span>Derived addresses · finalized reads · explicit refusals</span>
      <span>No index · no inferred holdings · raw u64 atoms</span>
    </footer>
  </main>;
}
