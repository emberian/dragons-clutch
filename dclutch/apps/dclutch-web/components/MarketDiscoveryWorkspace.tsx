'use client';

import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';
import MarketFilterBar from '@/components/MarketFilterBar';
import MarketIssuanceHistory from '@/components/charts/MarketIssuanceHistory';
import SupplyShareStrip from '@/components/charts/SupplyShareStrip';
import { useCallback, useEffect, useState, type ReactNode } from 'react';

import { type DeploymentV1 } from '@/lib/deployments';
import { useDeploymentV1 } from '@/lib/deploymentStore';
import { docsHrefV1 } from '@/lib/flags';
import {
  curateMarketListingV1,
  enumerateCoreMarketAddressesV1,
  inspectMarketDiscoveryV1,
  provenanceChipV1,
  shortAddressV1,
  type IncompatibleMarketAccountV1,
  type MarketCapabilityManifestV1,
  type MarketDiscoveryCardV1,
  type MarketDiscoveryV1,
  type MarketEnumerationV1,
  type MarketListingV1,
} from '@/lib/marketDiscovery';
import {
  filterMarketCardsV1,
  noMatchSentenceV1,
  sortMarketCardsV1,
  type MarketSortOrderV1,
} from '@/lib/marketFiltering';
import { marketDetailHrefV1 } from '@/lib/marketHref';
import { fallbackMarketTitleV1, marketEditorialV1 } from '@/lib/marketRegistry';
import { PUBLIC_DEVNET_CUT_V1 } from '@/lib/publicCutStaging';
import { SolanaRpcClient, type ConnectionFacts } from '@/lib/rpc';
import { clusterNameV1 } from '@/lib/rpcDefault';
import { deadlineMomentPhraseV1, readSlotClockV1, slotClockCaveatV1, type SlotClockV1 } from '@/lib/slotClock';
import { SUPPLY_SHARE_MEANING_V1 } from '@/lib/supplyShares';

type State =
  | Readonly<{ kind: 'loading' | 'refused'; message: string }>
  | Readonly<{ kind: 'ready'; message: string; discovery: MarketDiscoveryV1; facts: ConnectionFacts }>;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'discovery refused without a usable reason';
}

function plural(count: number, one: string, many: string): string {
  return count === 1 ? one : many;
}

/** Renders a deadline slot's wall-clock phrase once a clock is measured. */
type SlotClockPropsV1 = Readonly<{ clock?: SlotClockV1 | null; nowMs?: number | null }>;

function deadlinePhrase(badgeDeadline: string | null, clock: SlotClockV1 | null | undefined, nowMs: number | null | undefined): string {
  if (badgeDeadline === null || clock === undefined || clock === null || nowMs === undefined || nowMs === null) return '';
  return ` · ${deadlineMomentPhraseV1(clock, badgeDeadline, nowMs)}`;
}

function CapabilityBadges({ capabilities, clock, nowMs }: Readonly<{ capabilities: MarketCapabilityManifestV1 }> & SlotClockPropsV1) {
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
        <small>{badge.activation === 'deadline' ? `deadline ${badge.deadline}${deadlinePhrase(badge.deadline, clock, nowMs)}` : 'immediate'}{badge.dependencies.length > 0 ? ` · after ${badge.dependencies.join(', ')}` : ''}</small>
      </span>
    ))}
  </div>;
}

function MarketCard({ card, clock, nowMs }: Readonly<{ card: MarketDiscoveryCardV1 }> & SlotClockPropsV1) {
  if (card.status === 'refused') {
    return <article className="market-discovery-card refused">
      <div className="market-card-top"><span className="provenance-chip refused">{provenanceChipV1(card.provenance)}</span><span className="phase-chip">no phase</span></div>
      <h3><Anchor href={marketDetailHrefV1(card.address)} title={card.address}>{shortAddressV1(card.address, 10)}</Anchor></h3>
      <p className="market-refusal">{card.refusal}</p>
      <p className="market-observation">Finalized observation slot {card.observedSlot}</p>
    </article>;
  }
  // The name is the one editorial fact on the card: the chain stores no
  // titles, so a market the registry knows gets its registered name and
  // question, and one it does not gets a generated label that says exactly
  // what it is instead of pretending to a name.
  const editorial = marketEditorialV1(card.address);
  return <article className="market-discovery-card">
    <div className="market-card-top">
      <span className="provenance-chip">{provenanceChipV1(card.provenance)}</span>
      <span className={`phase-chip phase-${card.phase.toLowerCase()}`}>{card.phase}</span>
    </div>
    <h3><Anchor href={marketDetailHrefV1(card.address)} title={card.address}>{editorial === null ? fallbackMarketTitleV1(card.phase, card.address) : editorial.title}</Anchor></h3>
    {editorial !== null && <p className="market-question">{editorial.question}</p>}
    <p className="market-card-address" title={card.address}>{shortAddressV1(card.address, 10)}</p>
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
    {/* FE-CHART mount: the issuance split, drawn from the same supply vector
        the facts row above states exactly. */}
    {card.liability.status === 'bound' && <SupplyShareStrip
      supplies={card.liability.supplyAtoms}
      outcomes={editorial?.outcomes ?? null}
      caption={SUPPLY_SHARE_MEANING_V1}
      emptyReason="No claims have been issued on this market yet, so there is no split to draw."
    />}
    {/* FE-CHART mount: the recorded run, for the one market a run recorded.
        Every other card renders nothing here — a listing of empty frames
        would report a measurement nobody took. */}
    <MarketIssuanceHistory address={card.address} outcomes={editorial?.outcomes ?? null} />
    <p className="market-hoard-note">Supplies are the exact claim liabilities the Market&apos;s Claims aggregate records. They are not liquidity, TVL, or a balance available to any participant.</p>
    {card.hoard.status === 'derived'
      ? <p className="market-hoard-note">Hoard principal <strong>{card.hoard.principalAtoms}</strong> atoms{card.hoard.mintDisplayDecimals === null ? '' : ` · the mint prints ${card.hoard.mintDisplayDecimals} display decimals, which never scale this figure`}, held by this Market&apos;s Custody transfer authority at <span title={card.hoard.address}>{shortAddressV1(card.hoard.address)}</span>, in the Custody namespace the Claims aggregate records.</p>
      : <p className="market-capability-refusal"><span>Hoard {card.hoard.status}</span>{card.hoard.reason}</p>}
    <p className="market-observation"><Anchor href={marketDetailHrefV1(card.address)}>Open this Market field by field →</Anchor></p>
    {card.collateral.status !== 'bound' && <p className="market-refusal">{card.collateral.reason}</p>}
    {card.liability.status !== 'bound' && <p className="market-refusal">{card.liability.reason}</p>}
    <CapabilityBadges capabilities={card.capabilities} clock={clock} nowMs={nowMs} />
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

/**
 * One named, collapsed group of markets that are not the headline.
 *
 * Collapsed is not hidden. The summary carries the count and what happened to
 * them before anyone clicks, so a reader who never opens it still leaves
 * knowing exactly how many there are and why they are down here. That is the
 * whole point: the debris of a public build-out is part of the record, and the
 * choice is between framing it and pretending it is not there.
 */
function ListingGroup({
  title,
  note,
  children,
}: Readonly<{ title: string; note: string; children: ReactNode }>) {
  return <details className="listing-group">
    <summary><span>{title}</span><small>{note}</small></summary>
    <div>{children}</div>
  </details>;
}

/** The older-generation accounts, named and linked, never counted as current. */
export function HistoricalMarketAccounts({
  accounts,
}: Readonly<{ accounts: ReadonlyArray<IncompatibleMarketAccountV1> }>) {
  if (accounts.length === 0) return null;
  const count = accounts.length;
  return <ListingGroup
    title={`${count} older market${plural(count, '', 's')} this page cannot read`}
    note="disclosed here but not listed as current"
  >
    <p className="market-empty">
      {count} historical DCLTCOR2 Market account{plural(count, '', 's')} {plural(count, 'uses', 'use')} the old 352-byte
      layout. The current 360-byte reader will not guess at the difference, so it declines to decode
      {plural(count, ' it', ' them')} rather than show you a field it inferred. {plural(count, 'It is', 'They are')} still
      on the chain, and the explorer will still show you {plural(count, 'its', 'their')} bytes.
    </p>
    <ul className="market-bindings">
      {accounts.map((account) => <li key={account.address}>
        <Anchor href={`/explorer?view=account&q=${encodeURIComponent(account.address)}`} title={account.address}>{account.address}</Anchor>
        <small>{account.magic} · {account.accountBytes} bytes · historical and incompatible</small>
      </li>)}
    </ul>
  </ListingGroup>;
}

/**
 * Everything this deployment holds that is not an open market.
 *
 * Exported so the arrangement is pinned by a test rather than only by a
 * screenshot: each group has to keep its label and its count, and a founding
 * that never finished may never appear anywhere the open markets do.
 */
export function RestOfTheRecord({
  listing,
  incompatible,
  clock,
  nowMs,
}: Readonly<{
  listing: MarketListingV1;
  incompatible: ReadonlyArray<IncompatibleMarketAccountV1>;
}> & SlotClockPropsV1) {
  const founding = listing.founding.length;
  const settled = listing.settled.length;
  const unreadable = listing.unreadable.length;
  if (founding + settled + unreadable + incompatible.length === 0) return null;
  return <section className="trade-v3-card">
    <header>
      <span>02</span>
      <div><h2>The rest of the record</h2><p>Building a protocol on a public network leaves a public trail. These are the accounts this deployment holds that are not open markets, kept where they are and labelled for what they are. Nothing here is hidden; it is just not what you came for.</p></div>
    </header>

    {settled > 0 && <ListingGroup
      title={`${settled} market${plural(settled, '', 's')} past ${plural(settled, 'its', 'their')} answer`}
      note="resolved, retiring, or retired"
    >
      <div className="market-card-grid">{listing.settled.map((card) => <MarketCard key={card.address} card={card} clock={clock} nowMs={nowMs} />)}</div>
    </ListingGroup>}

    {founding > 0 && <ListingGroup
      title={`${founding} founding${plural(founding, '', 's')} that never finished`}
      note="started during the build-out · kept because devnet history is public"
    >
      <p className="market-empty">
        Founding a market takes a run of transactions, and {plural(founding, 'this one', 'these')} stopped part-way
        through. Each account still says exactly how far it got — its generation, its readiness, and the identities it
        had already committed to. {plural(founding, 'It sits', 'They sit')} apart from the open markets because there is
        nothing to trade against {plural(founding, 'it', 'them')}, not because {plural(founding, 'it is', 'they are')} something
        to be quiet about.
      </p>
      <div className="market-card-grid">{listing.founding.map((card) => <MarketCard key={card.address} card={card} clock={clock} nowMs={nowMs} />)}</div>
    </ListingGroup>}

    {unreadable > 0 && <ListingGroup
      title={`${unreadable} account${plural(unreadable, '', 's')} that refused to decode`}
      note="enumerated as current · each carries its exact reason"
    >
      <div className="market-card-grid">{listing.unreadable.map((card) => <MarketCard key={card.address} card={card} clock={clock} nowMs={nowMs} />)}</div>
    </ListingGroup>}

    <HistoricalMarketAccounts accounts={incompatible} />
  </section>;
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
  if (deployment.cluster === 'devnet') {
    return <div>
      <p className="market-empty">
        No current compatible market is listed on devnet at this finalized floor. When a current founding lands on this deployment, it appears here with zero configuration.{' '}
        <Anchor href={docsHrefV1('evidence/DEPLOY_1.html', 'docs/evidence/DEPLOY_1.md')}>Read the deployment evidence →</Anchor>
      </p>
      <HistoricalMarketAccounts accounts={incompatible} />
    </div>;
  }
  return <div>
    <p className="market-empty">
      No current compatible market is listed on this {deployment.label.toLowerCase()} deployment at the finalized floor.{' '}
      <Anchor href="/create">Preview a Market design →</Anchor>
    </p>
    <HistoricalMarketAccounts accounts={incompatible} />
  </div>;
}

export default function MarketDiscoveryWorkspace() {
  const deployment = useDeploymentV1();
  const [state, setState] = useState<State>({ kind: 'loading', message: 'Reading the finalized market list…' });
  const discovery = state.kind === 'ready' ? state.discovery : null;
  // The wall-clock layer: a slot-rate clock measured after each read, and a
  // ticking "now". Both start absent, so the server-rendered document and the
  // first client render carry no time strings at all — every estimate appears
  // only once it can actually be estimated.
  const [clock, setClock] = useState<SlotClockV1 | null>(null);
  const [nowMs, setNowMs] = useState<number | null>(null);
  // Narrowing state. Both start empty, so the page a reader lands on is the
  // whole listing in the chain's own order — the control changes what you see
  // only once you touch it, and never what the page reports exists.
  const [query, setQuery] = useState('');
  const [order, setOrder] = useState<MarketSortOrderV1>('enumerated');

  useEffect(() => {
    let cancelled = false;
    queueMicrotask(() => {
      if (!cancelled) setNowMs(Date.now());
    });
    const tick = setInterval(() => setNowMs(Date.now()), 30_000);
    return () => {
      cancelled = true;
      clearInterval(tick);
    };
  }, []);

  const load = useCallback(async () => {
    setState({ kind: 'loading', message: `Reading every ${deployment.label} market: one bounded finalized scan of the Core program, then every Market root, its Realm record, its Claims liability aggregate, and its capability manifest behind one finalized floor…` });
    setClock(null);
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
      // After the content is on screen: measure the cluster's slot rate so
      // deadline slots can carry a wall-clock phrase. Display-only, so its
      // failure mode is the labelled nominal-rate assumption, never an error.
      setClock(await readSlotClockV1(client, next.floorSlot));
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

  // The published cut names this deployment's headline Market. It only ever
  // reorders the open group; a featured address the chain does not say is open
  // is not promoted into it, because ordering is this page's to choose and
  // phase is not.
  const featured = deployment.cluster === 'devnet' ? PUBLIC_DEVNET_CUT_V1.market : null;
  // Two listings on purpose: the whole one, which is what this deployment
  // HOLDS and is what every count on the page reports, and the narrowed one,
  // which is only what the reader asked to look at. Conflating them is how a
  // search quietly becomes a claim that the hidden markets stopped existing.
  const wholeListing = discovery === null ? null : curateMarketListingV1(discovery.cards, featured);
  const matched = discovery === null ? null : filterMarketCardsV1(discovery.cards, query);
  const narrowed = matched === null ? null : curateMarketListingV1(matched, featured);
  const listing = narrowed === null ? null : {
    ...narrowed,
    open: sortMarketCardsV1(narrowed.open, order),
    settled: sortMarketCardsV1(narrowed.settled, order),
    founding: sortMarketCardsV1(narrowed.founding, order),
    unreadable: sortMarketCardsV1(narrowed.unreadable, order),
  };
  const searching = query.trim().length > 0;
  const incompatible = discovery !== null && discovery.enumeration.mode === 'program-scan'
    ? discovery.enumeration.incompatibleMarketAccounts
    : Object.freeze([]);
  const asideCount = wholeListing === null ? 0 : wholeListing.founding.length + wholeListing.settled.length + wholeListing.unreadable.length + incompatible.length;

  return <main className="product-shell trade-v3-shell">
    <Nav current="/markets" status={`${deployment.label} · finalized reads`} />

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">Markets on {deployment.label} · finalized reads only</p>
        <h1>Every market on devnet.<br /><em>Read live, or not at all.</em></h1>
        <p>The markets that are open come first, because they are the ones you can do something with. Everything else this deployment holds — foundings that were started and never finished, and markets from an older version of the protocol that this build cannot decode — is counted and named below rather than dropped. Each card shows only what the chain actually says: what phase the market is in and who it commits to, how many claims exist and what is holding them, what it is collateralized in, and what the market is allowed to do. There is no volume, price, odds, probability, or yield here, because the chain does not store any of those. What it does store is how many claims of each outcome have been issued, so each card draws that split for exactly what it is — where the issued claims sit, never a forecast.</p>
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
        <div><h2>The markets that are open</h2><p>One card per market that has finished founding, enumerated from the Core program itself — no index, no curation of which facts you see. A card is either read or refused; it is never partly invented. Claim counts come from the accounts that actually hold the claims, in raw units. The name and question at the top of a card are this site&apos;s editorial — the chain stores no names; everything else on the card is read from the chain.</p></div>
        <div className="direct-actions"><button type="button" onClick={() => void load()} disabled={state.kind === 'loading'}>{state.kind === 'loading' ? 'Reading…' : 'Re-read the chain'}</button></div>
      </header>
      {state.kind === 'refused'
        ? <p className="market-refusal" aria-live="polite">{state.message}</p>
        : <p className="direct-status" aria-live="polite">{state.message}</p>}
      {discovery !== null && listing !== null && state.kind === 'ready' && <>
        <div className="trade-v3-evidence">
          <article><span>Endpoint</span><strong>{state.facts.solanaCore}</strong><small>{clusterNameV1(state.facts.genesisHash)} · genesis {shortAddressV1(state.facts.genesisHash, 6)}</small></article>
          <article><span>Finalized floor</span><strong>{discovery.floorSlot}</strong><small>{clock === null ? 'one observation epoch for every card' : `read at ${new Date(clock.observedAtMs).toLocaleTimeString()} · one observation epoch for every card`}</small></article>
          <article><span>Open now</span><strong>{wholeListing === null ? '—' : wholeListing.open.length}</strong><small>{asideCount} further account{plural(asideCount, '', 's')} named below</small></article>
          <article><span>Core program</span><strong>{shortAddressV1(deployment.programs.core, 6)}</strong><small>{deployment.cluster === 'devnet' ? 'DEPLOY-1 permanent address' : 'the active deployment'}</small></article>
        </div>
        <p className="direct-status">{discovery.enumeration.note}</p>
        {clock !== null && <p className="slot-clock-note">{slotClockCaveatV1(clock)}</p>}
        {discovery.enumeration.mode === 'refused' && <p className="market-refusal">{discovery.enumeration.reason}</p>}
        {discovery.cards.length > 0 && <MarketFilterBar
          query={query}
          onQuery={setQuery}
          order={order}
          onOrder={setOrder}
          shown={matched === null ? 0 : matched.length}
          total={discovery.cards.length}
        />}
        {discovery.cards.length === 0
          ? discovery.enumeration.mode === 'refused' ? null : <EmptyMarkets deployment={deployment} enumeration={discovery.enumeration} />
          : listing.open.length === 0
            ? searching && wholeListing !== null && wholeListing.open.length > 0
              ? <p className="market-empty">{noMatchSentenceV1(query, wholeListing.open.length)}</p>
              : <p className="market-empty">Nothing on this deployment has finished founding yet. Every market it holds is named below, at the stage it actually reached.</p>
            : <div className="market-card-grid">{listing.open.map((card) => <MarketCard key={card.address} card={card} clock={clock} nowMs={nowMs} />)}</div>}
      </>}
    </section>

    {discovery !== null && listing !== null && state.kind === 'ready'
      && <RestOfTheRecord listing={listing} incompatible={incompatible} clock={clock} nowMs={nowMs} />}

    <footer className="product-footer">
      <span>Chain-derived phase, atoms, and refusals only</span>
      <span>No volume · no odds · no probability · no yield</span>
      <span>Issuance shares are issuance, not odds</span>
    </footer>
  </main>;
}
