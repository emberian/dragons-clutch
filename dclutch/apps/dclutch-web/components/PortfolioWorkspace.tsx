'use client';

import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';
import { FormEvent, useCallback, useState } from 'react';

import PositionBars from '@/components/charts/PositionBars';
import RedeemFlow from '@/components/RedeemFlow';
import WalletDirectory, { useWalletDirectoryV1, type WalletDirectoryHandleV1 } from '@/components/WalletDirectory';
import { useDeploymentV1 } from '@/lib/deploymentStore';
import {
  enumerateCoreMarketAddressesV1,
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
import { clusterNameV1 } from '@/lib/rpcDefault';
import { marketDetailHrefV1 } from '@/lib/marketHref';

type State =
  | Readonly<{ kind: 'idle' | 'loading' | 'refused'; message: string }>
  | Readonly<{ kind: 'ready'; message: string; portfolio: PortfolioV1; facts: ConnectionFacts }>;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'the portfolio read refused without a usable reason';
}

function MarketHeading({ market, address }: Readonly<{ market: MarketDiscoveryCardV1; address: string }>) {
  return <div className="market-card-top">
    <Anchor href={marketDetailHrefV1(address)} title={address}>{shortAddressV1(address, 8)}</Anchor>
    <span className={`provenance-chip${market.provenance.kind === 'refused' ? ' refused' : ''}`}>{provenanceChipV1(market.provenance)}</span>
    <span className={`phase-chip${market.status === 'decoded' ? ` phase-${market.phase.toLowerCase()}` : ''}`}>{market.status === 'decoded' ? market.phase : 'no phase'}</span>
  </div>;
}

type RedeemContextV1 = Readonly<{
  endpoint: string;
  claimsProgramId: string;
  custodyProgramId: string;
  registryProgramId: string;
  directory: WalletDirectoryHandleV1;
}>;

function PositionEntry({ entry, redeem }: Readonly<{ entry: PortfolioEntryV1; redeem: RedeemContextV1 }>) {
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
      {/* FE-CHART mount: the same balances as bars, with the phase's own line
          through them; the list below stays as the exact-value twin. */}
      <PositionBars
        balances={position.balances}
        claim={position.claim.kind === 'mergeable'
          ? { kind: 'mergeable', completeSetsAtoms: position.claim.completeSetsAtoms }
          : position.claim.kind === 'redeemable'
            ? { kind: 'redeemable', winningClaim: position.claim.winningClaim, redeemableAtoms: position.claim.redeemableAtoms }
            : { kind: 'unavailable' }}
        caption="Heights are owned claim atoms per claim, read finalized from this Position."
      />
      <ol className="outcome-vector">
        {position.balances.map((amount, index) => (
          <li key={index} className={position.claim.kind === 'redeemable' && position.claim.winningClaim === index ? 'winning-outcome' : ''}>
            <span>claim {index}</span>
            <strong>{amount}</strong>
            {position.claim.kind === 'redeemable' && <small>{position.claim.winningClaim === index ? `winning · ${position.claim.redeemableAtoms} atoms admitted to redemption` : 'losing · pays zero'}</small>}
          </li>
        ))}
      </ol>
      {position.claim.kind === 'mergeable' && <div className="portfolio-claim">
        <span>Complete sets mergeable</span>
        <strong>{position.claim.completeSetsAtoms}</strong>
        <p>{position.claim.note}</p>
      </div>}
      {position.claim.kind === 'redeemable' && <div className="portfolio-claim">
        <span>Winning-claim atoms admitted to redemption</span>
        <strong>{position.claim.redeemableAtoms}</strong>
        <p>{position.claim.note}</p>
      </div>}
      {position.claim.kind === 'redeemable' && <RedeemFlow
        endpoint={redeem.endpoint}
        marketAddress={entry.marketAddress}
        positionAddress={position.address}
        claimIndex={position.claim.winningClaim}
        availableQuantity={position.claim.redeemableAtoms}
        claimsProgramId={redeem.claimsProgramId}
        custodyProgramId={redeem.custodyProgramId}
        registryProgramId={redeem.registryProgramId}
        directory={redeem.directory}
      />}
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

export default function PortfolioWorkspace({ mode = 'portfolio' }: Readonly<{ mode?: 'portfolio' | 'redemption' }>) {
  const deployment = useDeploymentV1();
  const directory = useWalletDirectoryV1();
  const redemption = mode === 'redemption';
  const [owner, setOwner] = useState('');
  const [pasted, setPasted] = useState('');
  const [state, setState] = useState<State>({
    kind: 'idle',
    message: redemption
      ? 'Connect your wallet. This page then reads its Positions across every current-compatible Market of the active deployment and shows a redemption control only for an exact winning balance.'
      : 'Connect a wallet — or paste any owner address — and this surface reads its Positions across every Market of the active deployment. Reading a derived address requires no authority at all.',
  });
  const portfolio = state.kind === 'ready' ? state.portfolio : null;

  const read = useCallback(async (ownerAddress: string) => {
    setState({ kind: 'loading', message: 'Enumerating the deployment’s Markets, deriving one Claims aggregate and one Position address per Market for this owner, then reading every derived address behind one finalized floor…' });
    try {
      const client = new SolanaRpcClient(deployment.endpoint);
      const facts = await client.probe();
      const enumeration = await enumerateCoreMarketAddressesV1(client, deployment.programs.core);
      const next = await inspectPortfolioV1(client, {
        coreProgramId: deployment.programs.core,
        claimsProgramId: deployment.programs.claims,
        registryProgramId: deployment.programs.registry,
        owner: parsePortfolioOwnerV1(ownerAddress),
        marketAddresses: enumeration.addresses.slice(0, PORTFOLIO_MAX_MARKETS),
      });
      setState({ kind: 'ready', portfolio: next, facts, message: enumeration.mode === 'refused' ? `${next.reason} (${enumeration.reason})` : next.reason });
    } catch (error) {
      setState({ kind: 'refused', message: `Refused: ${errorMessage(error)}` });
    }
  }, [deployment]);

  function connected(address: string) {
    setOwner(address);
    // Auto-load on wallet connect: the owner was the ONLY missing input.
    void read(address);
  }

  function readPasted(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const candidate = pasted.trim();
    if (candidate === '') return;
    setOwner(candidate);
    void read(candidate);
  }

  return <main className="product-shell trade-v3-shell">
    <Nav current={redemption ? '/redeem' : '/portfolio'} status={`${deployment.label} · ${redemption ? 'wallet redemption' : 'finalized reads'}`} />

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">{redemption ? 'Redeem · connected wallet, exact Positions' : 'Portfolio · derived addresses, no index'}</p>
        <h1>{redemption ? <>Redeem your winning claims.<br /><em>Only when the chain says you can.</em></> : <>Your claims, derived.<br /><em>Never looked up.</em></>}</h1>
        <p>{redemption
          ? <>Connect your wallet and this page reads every current-compatible Market from the deployment&apos;s Core program, derives your exact Claims Position under the selected Claims program, and offers redemption only when finalized Market state names a winner and your Position holds that winning claim. No Market, Position, balance, or eligibility comes from browser storage.</>
          : <>dClutch runs no indexer and this browser will not pretend to be one. A Position lives at the program-derived address of the Position seed domain plus the exact Market and owner keys, so an owner identity is enough: the Markets come from the deployment&apos;s own Core program, and every Position address is derived from them. A derived address that holds no account is reported as exactly that, which is the honest chain state.</>}</p>
      </div>
      <aside>
        <span>The one input</span>
        <strong>{redemption ? 'Your wallet identity' : 'An owner identity'}</strong>
        <p>{redemption
          ? 'Connecting first reads only your public address. A signature is a later, separate action that appears only after the chain and a Rust-authored payout plan agree on the exact Market, Position, owner, winning claim, recipient, programs, and lookup table.'
          : 'Connecting a wallet reads a public address and nothing else — no signature, no approval. Pasting any address reads the same finalized state for that owner, because reading a derived address requires no authority at all.'}</p>
      </aside>
    </section>

    <section className="trade-v3-card route-card">
      <header><span>01</span><div><h2>{redemption ? 'Connect your wallet' : 'Whose Positions?'}</h2><p>Everything else — endpoint, Core authority, Claims program, the Market list — comes from the active {deployment.label} deployment. This surface asks only who you are.</p></div></header>
      <WalletDirectory directory={directory} purpose={redemption ? 'find and redeem your winning claims' : 'read one owner identity'} onConnected={connected} />
      {!redemption && <form className="portfolio-owner-row" onSubmit={readPasted}>
        <label><span>Or paste any owner address</span><input value={pasted} onChange={(event) => setPasted(event.target.value.trim())} spellCheck={false} placeholder="an owner’s public address" /></label>
        <div className="direct-actions">
          <button disabled={state.kind === 'loading'}>{state.kind === 'loading' ? 'Reading…' : 'Read this owner’s Positions'}</button>
          {owner !== '' && state.kind !== 'loading' && <button type="button" className="secondary-action" onClick={() => void read(owner)}>Re-read</button>}
        </div>
      </form>}
      {redemption && <p className="direct-status">This wallet path permanently refuses Solana mainnet, testnet, and unknown non-local chains before it asks for a signature. On devnet it still signs nothing until the current Market, your derived Position, every named program and account, and the exact payout plan all pass finalized preflight. The payout plan is still produced outside this browser; this page does not invent one from partial state.</p>}
      <p className="direct-status" aria-live="polite">{state.message}</p>
    </section>

    <section className="trade-v3-card">
      <header><span>02</span><div><h2>{redemption ? 'Your redeemable Positions' : 'Derived Positions'}</h2><p>One entry per Market of the deployment. Every entry states the address that was derived, whether an account exists there, and what the balances found admit under the Market&apos;s own phase and settlement.</p></div></header>
      {portfolio === null && <p className="market-empty">No finalized Position state has been read yet. Until an owner identity arrives and the endpoint answers, this surface stays empty rather than showing placeholder holdings.</p>}
      {portfolio !== null && state.kind === 'ready' && <>
        <div className="trade-v3-evidence">
          <article><span>Owner</span><strong>{shortAddressV1(portfolio.owner, 6)}</strong><small>{redemption ? 'connected wallet; signing remains a separate action' : 'identity only; nothing is signed here'}</small></article>
          <article><span>Finalized floor</span><strong>{portfolio.floorSlot}</strong><small>one observation epoch for every entry</small></article>
          <article><span>Endpoint</span><strong>{state.facts.solanaCore}</strong><small>{clusterNameV1(state.facts.genesisHash)} · genesis {shortAddressV1(state.facts.genesisHash, 6)}</small></article>
          <article><span>Derived addresses</span><strong>{portfolio.entries.length}</strong><small>one per Market of the deployment</small></article>
        </div>
        {portfolio.entries.length === 0
          ? <p className="market-empty">{portfolio.reason}</p>
          : <div className="market-card-grid">{portfolio.entries.map((entry) => <PositionEntry key={entry.marketAddress} entry={entry} redeem={{ endpoint: deployment.endpoint, claimsProgramId: deployment.programs.claims, custodyProgramId: deployment.programs.custody, registryProgramId: deployment.programs.registry, directory }} />)}</div>}
      </>}
    </section>

    <footer className="product-footer">
      <span>{redemption ? 'Connected owner · finalized preflight · recoverable submission' : 'Derived addresses · finalized reads · explicit refusals'}</span>
      <span>{redemption ? 'No mainnet · no invented plan · no ambiguous replay' : 'No index · no inferred holdings · raw u64 atoms'}</span>
    </footer>
  </main>;
}
