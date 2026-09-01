'use client';

import PageShell from '@/components/PageShell';
import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';
import { FormEvent, useCallback, useState } from 'react';

import BundleExposurePanel from '@/components/BundleExposurePanel';
import PositionBars from '@/components/charts/PositionBars';
import RedeemFlow from '@/components/RedeemFlow';
import WalletDirectory, { useWalletDirectoryV1, type WalletDirectoryHandleV1 } from '@/components/WalletDirectory';
import { bundleExposureV1 } from '@/lib/bundleExposure';
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
      <div><dt>Claims ledger for this market</dt><dd title={entry.aggregateAddress ?? undefined}>{entry.aggregateAddress === null ? 'could not be worked out' : shortAddressV1(entry.aggregateAddress, 8)}</dd></div>
      <div><dt>Where your claims would sit</dt><dd title={entry.positionAddress ?? undefined}>{entry.positionAddress === null ? 'could not be worked out' : shortAddressV1(entry.positionAddress, 8)}</dd></div>
      <div><dt>Read at finalized slot</dt><dd>{position.observedSlot}</dd></div>
      {market.status === 'decoded' && <div><dt>Market generation</dt><dd>{market.generation}</dd></div>}
      {market.status === 'decoded' && market.collateral.status === 'bound' && <div><dt>Paid out in</dt><dd title={market.collateral.collateralMint}>{market.collateral.collateralMintShort}</dd></div>}
    </dl>
    <div className="portfolio-position-provenance">
      <span className={`provenance-chip${position.provenance.kind === 'refused' ? ' refused' : ''}`}>{provenanceChipV1(position.provenance)}</span>
      <small>Position</small>
    </div>
    {position.status === 'absent' && <p className="market-empty">{position.note}</p>}
    {position.status === 'refused' && <p className="market-refusal">{position.reason}</p>}
    {position.status === 'held' && <>
      <h4 className="detail-subhead">What this wallet holds, per outcome</h4>
      {/* FE-CHART mount: the same balances as bars, with the phase's own line
          through them; the list below stays as the exact-value twin. */}
      <PositionBars
        balances={position.balances}
        claim={position.claim.kind === 'mergeable'
          ? { kind: 'mergeable', completeSetsAtoms: position.claim.completeSetsAtoms }
          : position.claim.kind === 'redeemable'
            ? { kind: 'redeemable', winningClaim: position.claim.winningClaim, redeemableAtoms: position.claim.redeemableAtoms }
            : { kind: 'unavailable' }}
        caption="Claims held per outcome."
      />
      <ol className="outcome-vector">
        {position.balances.map((amount, index) => (
          <li key={index} className={position.claim.kind === 'redeemable' && position.claim.winningClaim === index ? 'winning-outcome' : ''}>
            <span>claim {index}</span>
            <strong>{amount}</strong>
            {position.claim.kind === 'redeemable' && <small>{position.claim.winningClaim === index ? `won · ${position.claim.redeemableAtoms} can be cashed in` : 'lost · pays nothing'}</small>}
          </li>
        ))}
      </ol>
      {position.claim.kind === 'mergeable' && <div className="portfolio-claim">
        <span>Complete sets you could hand back</span>
        <strong>{position.claim.completeSetsAtoms}</strong>
        <p>{position.claim.note}</p>
      </div>}
      {position.claim.kind === 'redeemable' && <div className="portfolio-claim">
        <span>Winning claims you can cash in</span>
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
      {position.claim.kind === 'unavailable' && <p className="market-capability-refusal"><span>nothing you can do right now</span>{position.claim.note}</p>}
      <dl className="market-card-facts">
        <div><dt>Ledger this Position names</dt><dd title={position.aggregateAddress}>{shortAddressV1(position.aggregateAddress, 8)}</dd></div>
        <div><dt>Position revision</dt><dd>{position.revision}</dd></div>
        <div><dt>Outcomes</dt><dd>{position.claimCount}</dd></div>
        <div><dt>Rule it pays by</dt><dd title={position.liabilityBasisId}>{position.liabilityBasisId.slice(0, 16)}…</dd></div>
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
      ? 'Connect your wallet to find the claims it holds.'
      : 'Connect a wallet, or paste any address.',
  });
  const portfolio = state.kind === 'ready' ? state.portfolio : null;

  const read = useCallback(async (ownerAddress: string) => {
    setState({ kind: 'loading', message: 'Reading this owner’s claims in every market…' });
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

  return <PageShell className="product-shell trade-v3-shell" header={<Nav current={redemption ? '/redeem' : '/portfolio'} status={`${deployment.label} · ${redemption ? 'wallet redemption' : 'read live'}`} />}>

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">{redemption ? 'Redeem · your connected wallet, exactly what it holds' : 'Portfolio · what one wallet holds'}</p>
        <h1>{redemption ? <>Your winning claims.<br /><em>Payout is not open yet.</em></> : <>Everything one wallet<br /><em>holds here.</em></>}</h1>
        <p>{redemption
          ? <>Connect your wallet to find every claim it holds. Paying winning claims out is not available yet: when a market resolves and you hold the winning side, this is where you will do it.</>
          : <>Paste an address, or connect a wallet, to see what claims it holds in every market on this deployment.</>}</p>
      </div>
      <aside>
        <span>All this page needs</span>
        <strong>{redemption ? 'Your wallet' : 'One address'}</strong>
        <p>{redemption
          ? 'Connecting reads your address. Signing is a separate step, later.'
          : 'Connecting reads your address. Nothing is signed.'}</p>
      </aside>
    </section>

    <section className="trade-v3-card route-card">
      <header><span>01</span><div><h2>{redemption ? 'Connect your wallet' : 'Whose wallet?'}</h2></div></header>
      <WalletDirectory directory={directory} purpose={redemption ? 'find the winning claims you hold' : 'read one owner identity'} onConnected={connected} />
      {!redemption && <form className="portfolio-owner-row" onSubmit={readPasted}>
        <label><span>Or paste any owner address</span><input value={pasted} onChange={(event) => setPasted(event.target.value.trim())} spellCheck={false} placeholder="an owner’s public address" /></label>
        <div className="direct-actions">
          <button disabled={state.kind === 'loading'}>{state.kind === 'loading' ? 'Reading…' : 'See what this address holds'}</button>
          {owner !== '' && state.kind !== 'loading' && <button type="button" className="secondary-action" onClick={() => void read(owner)}>Re-read</button>}
        </div>
      </form>}
      {redemption && <p className="direct-status">Devnet only — mainnet and testnet are refused.</p>}
      <p className="direct-status" aria-live="polite">{state.message}</p>
    </section>

    <section className="trade-v3-card">
      <header><span>02</span><div><h2>Across everything you hold</h2><p>The most and the least all of it can pay, added up.</p></div></header>
      {portfolio === null
        ? <p className="market-empty">Nothing read yet.</p>
        : <BundleExposurePanel exposure={bundleExposureV1(portfolio)} />}
    </section>

    <section className="trade-v3-card">
      <header><span>03</span><div><h2>{redemption ? 'What you can cash in' : 'Market by market'}</h2><p>One entry per market on this deployment.</p></div></header>
      {portfolio === null && <p className="market-empty">Nothing read yet.</p>}
      {portfolio !== null && state.kind === 'ready' && <>
        <div className="trade-v3-evidence">
          <article><span>Wallet</span><strong>{shortAddressV1(portfolio.owner, 6)}</strong><small>{redemption ? 'connected' : 'address only'}</small></article>
          <article><span>Finalized floor</span><strong>{portfolio.floorSlot}</strong><small>slot</small></article>
          <article><span>Endpoint</span><strong>{state.facts.solanaCore}</strong><small>{clusterNameV1(state.facts.genesisHash)} · genesis {shortAddressV1(state.facts.genesisHash, 6)}</small></article>
          <article><span>Markets checked</span><strong>{portfolio.entries.length}</strong><small>on this deployment</small></article>
        </div>
        {portfolio.entries.length === 0
          ? <p className="market-empty">{portfolio.reason}</p>
          : <div className="market-card-grid">{portfolio.entries.map((entry) => <PositionEntry key={entry.marketAddress} entry={entry} redeem={{ endpoint: deployment.endpoint, claimsProgramId: deployment.programs.claims, custodyProgramId: deployment.programs.custody, registryProgramId: deployment.programs.registry, directory }} />)}</div>}
      </>}
    </section>

  </PageShell>;
}
