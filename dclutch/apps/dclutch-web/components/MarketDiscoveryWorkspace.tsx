'use client';

import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';
import { useCallback, useEffect, useState } from 'react';

import { type DeploymentV1 } from '@/lib/deployments';
import { useDeploymentV1 } from '@/lib/deploymentStore';
import { docsHrefV1 } from '@/lib/flags';
import {
  enumerateCoreMarketAddressesV1,
  inspectMarketDiscoveryV1,
  provenanceChipV1,
  shortAddressV1,
  type MarketCapabilityManifestV1,
  type MarketDiscoveryCardV1,
  type MarketDiscoveryV1,
  type MarketEnumerationV1,
} from '@/lib/marketDiscovery';
import { marketDetailHrefV1 } from '@/lib/marketHref';
import { SolanaRpcClient, type ConnectionFacts } from '@/lib/rpc';
import { clusterNameV1 } from '@/lib/rpcDefault';

type State =
  | Readonly<{ kind: 'loading' | 'refused'; message: string }>
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
      <h3><Anchor href={marketDetailHrefV1(card.address)} title={card.address}>{shortAddressV1(card.address, 10)}</Anchor></h3>
      <p className="market-refusal">{card.refusal}</p>
      <p className="market-observation">Finalized observation slot {card.observedSlot}</p>
    </article>;
  }
  return <article className="market-discovery-card">
    <div className="market-card-top">
      <span className="provenance-chip">{provenanceChipV1(card.provenance)}</span>
      <span className={`phase-chip phase-${card.phase.toLowerCase()}`}>{card.phase}</span>
    </div>
    <h3><Anchor href={marketDetailHrefV1(card.address)} title={card.address}>{shortAddressV1(card.address, 10)}</Anchor></h3>
    <dl className="market-card-facts">
      <div><dt>Generation</dt><dd>{card.generation}</dd></div>
      <div><dt>Founding readiness</dt><dd>{card.readiness}</dd></div>
      <div><dt>Outstanding capabilities</dt><dd>{card.outstandingCapabilities}</dd></div>
      <div><dt>Claim count · Claims aggregate</dt><dd>{card.liability.status === 'bound' ? card.liability.claimCount : card.liability.status}</dd></div>
      <div><dt>Per-claim supply · raw u64</dt><dd>{card.liability.status === 'bound' ? card.liability.supplyAtoms.join(' · ') : card.liability.status}</dd></div>
      <div><dt>Exact required backing · raw u64</dt><dd>{card.liability.status === 'bound' ? card.liability.requiredBackingAtoms : card.liability.status}</dd></div>
      <div><dt>Terminal receipt</dt><dd>{card.settlement.status === 'terminal'
        ? `${card.settlement.label} · winning claim ${card.settlement.winner}`
        : card.settlement.label}</dd></div>
      <div><dt>Collateral mint</dt><dd>{card.collateral.status === 'bound'
        ? <span title={card.collateral.collateralMint}>{card.collateral.collateralMintShort}</span>
        : card.collateral.status}</dd></div>
      <div><dt>Realm content ID</dt><dd title={card.identity.realmId}>{card.identity.realmId.slice(0, 16)}…</dd></div>
      <div><dt>Finalized observed slot</dt><dd>{card.observedSlot}</dd></div>
    </dl>
    <p className="market-hoard-note">Supplies are the exact claim liabilities the Market&apos;s Claims aggregate records. They are not liquidity, TVL, or a balance available to any participant.</p>
    {card.hoard.status === 'derived'
      ? <p className="market-hoard-note">Hoard principal <strong>{card.hoard.principalAtoms}</strong> atoms, held by this Market&apos;s Custody transfer authority at <span title={card.hoard.address}>{shortAddressV1(card.hoard.address)}</span>, in the Custody namespace the Claims aggregate records.</p>
      : <p className="market-capability-refusal"><span>Hoard {card.hoard.status}</span>{card.hoard.reason}</p>}
    <p className="market-observation"><Anchor href={marketDetailHrefV1(card.address)}>Open this Market field by field →</Anchor></p>
    {card.collateral.status !== 'bound' && <p className="market-refusal">{card.collateral.reason}</p>}
    {card.liability.status !== 'bound' && <p className="market-refusal">{card.liability.reason}</p>}
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

/** The cluster-true empty state: a fact and a link, never a form. */
export function EmptyMarkets({
  deployment,
  enumeration,
}: Readonly<{
  deployment: DeploymentV1;
  enumeration: MarketEnumerationV1;
}>) {
  const incompatible = enumeration.mode === 'program-scan'
    ? enumeration.incompatibleMarketAccounts
    : Object.freeze([]);
  const incompatibleDisclosure = incompatible.length === 0 ? null : <>
    <p className="market-empty">
      This scan also found {incompatible.length} historical DCLTCOR2 Market account{incompatible.length === 1 ? '' : 's'}.
      {` ${incompatible.length === 1 ? 'It uses' : 'They use'} the old 352-byte layout, which the current 360-byte reader cannot decode, so ${incompatible.length === 1 ? 'it is' : 'they are'} disclosed here but not listed as current.`}
    </p>
    <details className="direct-details">
      <summary>Show the historical account{incompatible.length === 1 ? '' : 's'}</summary>
      <ul className="market-bindings">
        {incompatible.map((account) => <li key={account.address}>
          <Anchor href={`/explorer?view=account&q=${encodeURIComponent(account.address)}`} title={account.address}>{account.address}</Anchor>
          <small>{account.magic} · {account.accountBytes} bytes · historical and incompatible</small>
        </li>)}
      </ul>
    </details>
  </>;
  if (deployment.cluster === 'devnet') {
    return <div>
      <p className="market-empty">
        No current compatible market is listed on devnet at this finalized floor. When a current founding lands on this deployment, it appears here with zero configuration.{' '}
        <Anchor href={docsHrefV1('evidence/DEPLOY_1.html', 'docs/evidence/DEPLOY_1.md')}>Read the deployment evidence →</Anchor>
      </p>
      {incompatibleDisclosure}
    </div>;
  }
  return <div>
    <p className="market-empty">
      No current compatible market is listed on this {deployment.label.toLowerCase()} deployment at the finalized floor.{' '}
      <Anchor href="/create">Preview a Market design →</Anchor>
    </p>
    {incompatibleDisclosure}
  </div>;
}

export default function MarketDiscoveryWorkspace() {
  const deployment = useDeploymentV1();
  const [state, setState] = useState<State>({ kind: 'loading', message: 'Reading the finalized market list…' });
  const discovery = state.kind === 'ready' ? state.discovery : null;

  const load = useCallback(async () => {
    setState({ kind: 'loading', message: `Reading every ${deployment.label} market: one bounded finalized scan of the Core program, then every Market root, its Realm record, its Claims liability aggregate, and its capability manifest behind one finalized floor…` });
    try {
      const client = new SolanaRpcClient(deployment.endpoint);
      const facts = await client.probe();
      const enumeration = await enumerateCoreMarketAddressesV1(client, deployment.programs.core);
      const next = await inspectMarketDiscoveryV1(client, {
        coreProgramId: deployment.programs.core,
        registryProgramId: deployment.programs.registry,
        claimsProgramId: deployment.programs.claims,
        custodyProgramId: deployment.programs.custody,
        addresses: enumeration.addresses,
        enumeration: enumeration.mode === 'program-scan' ? enumeration : undefined,
      });
      setState({ kind: 'ready', discovery: next, facts, message: next.reason });
    } catch (error) {
      setState({ kind: 'refused', message: `Refused: ${errorMessage(error)}` });
    }
  }, [deployment]);

  // Content on load: the market list is the page, not a reward for filling in
  // a form. Re-reads when the cluster picker changes the deployment.
  useEffect(() => {
    let cancelled = false;
    queueMicrotask(() => {
      if (!cancelled) void load();
    });
    return () => {
      cancelled = true;
    };
  }, [load]);

  return <main className="product-shell trade-v3-shell">
    <Nav current="/markets" status={`${deployment.label} · finalized reads`} />

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">Markets on {deployment.label} · finalized reads only</p>
        <h1>Every market on devnet.<br /><em>Read live, or not at all.</em></h1>
        <p>This is the whole current-compatible market list of the {deployment.label} deployment, enumerated from the Core program itself — no index, no curation. Older markets this build cannot read are listed separately rather than hidden. Each card shows only what the chain actually says: what phase the market is in and who it commits to, how many claims exist and what is holding them, what it is collateralized in, and what the market is allowed to do. There is no volume, price, odds, probability, or yield here, because the chain does not store any of those.</p>
      </div>
      <aside>
        <span>Provenance</span>
        <strong>CHAIN · finalized slot</strong>
        <p>Each surface carries its own provenance chip. A surface that cannot be decoded or bound carries REFUSED and its exact reason instead of a blank or a zero.</p>
      </aside>
    </section>

    <section className="trade-v3-card">
      <header>
        <span>01</span>
        <div><h2>The current markets</h2><p>One card per current-compatible Market the Core program owns at one finalized floor. Historical incompatible accounts are counted and disclosed separately. A card is decoded or refused; it is never partially invented. Supplies come from the Claims aggregate, never from the root, in raw u64 atoms.</p></div>
        <div className="direct-actions"><button type="button" onClick={() => void load()} disabled={state.kind === 'loading'}>{state.kind === 'loading' ? 'Reading…' : 'Re-read the chain'}</button></div>
      </header>
      {state.kind === 'refused'
        ? <p className="market-refusal" aria-live="polite">{state.message}</p>
        : <p className="direct-status" aria-live="polite">{state.message}</p>}
      {discovery !== null && state.kind === 'ready' && <>
        <div className="trade-v3-evidence">
          <article><span>Endpoint</span><strong>{state.facts.solanaCore}</strong><small>{clusterNameV1(state.facts.genesisHash)} · genesis {shortAddressV1(state.facts.genesisHash, 6)}</small></article>
          <article><span>Finalized floor</span><strong>{discovery.floorSlot}</strong><small>one observation epoch for every card</small></article>
          <article><span>Enumeration</span><strong>{discovery.enumeration.mode}</strong><small>{discovery.enumeration.addresses.length} current address{discovery.enumeration.addresses.length === 1 ? '' : 'es'}{discovery.enumeration.mode === 'program-scan' ? ` · ${discovery.enumeration.incompatibleMarketAccounts.length} historical incompatible` : ''}</small></article>
          <article><span>Core program</span><strong>{shortAddressV1(deployment.programs.core, 6)}</strong><small>{deployment.cluster === 'devnet' ? 'DEPLOY-1 permanent address' : 'the active deployment'}</small></article>
        </div>
        <p className="direct-status">{discovery.enumeration.note}</p>
        {discovery.enumeration.mode === 'refused' && <p className="market-refusal">{discovery.enumeration.reason}</p>}
        {discovery.cards.length === 0
          ? discovery.enumeration.mode === 'refused' ? null : <EmptyMarkets deployment={deployment} enumeration={discovery.enumeration} />
          : <div className="market-card-grid">{discovery.cards.map((card) => <MarketCard key={card.address} card={card} />)}</div>}
      </>}
    </section>

    <footer className="product-footer">
      <span>Chain-derived phase, atoms, and refusals only</span>
      <span>No volume · no odds · no probability · no yield</span>
    </footer>
  </main>;
}
