'use client';

import PageShell from '@/components/PageShell';
import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';
import MarketFilterBar from '@/components/MarketFilterBar';
import MarketIssuanceHistory from '@/components/charts/MarketIssuanceHistory';
import SupplyShareStrip from '@/components/charts/SupplyShareStrip';
import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from 'react';

import { CORE_STATE_BYTES } from '@/lib/generated/coreFound';
import { type DeploymentV1 } from '@/lib/deployments';
import { SUPERSEDED_CORE_STATE_WIDTHS } from '@/lib/marketCoreV2';
import { collateralDenominationV1 } from '@/lib/marketDenomination';
import { inspectMarketQuestionsV1, type MarketQuestionV1 } from '@/lib/marketQuestion';
import { formatQuantityV1 } from '@/lib/quantity';
import { useDeploymentV1 } from '@/lib/deploymentStore';
import { docsHrefV1 } from '@/lib/flags';
import {
  curateMarketListingV1,
  enumerateCoreMarketAddressesV1,
  inspectMarketDiscoveryV1,
  marketActivationOutlookV1,
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
import { marketEditorialV1, marketNarrativeV1 } from '@/lib/marketRegistry';
import { PUBLIC_DEVNET_CUT_V1 } from '@/lib/publicCutStaging';
import { SolanaRpcClient, type ConnectionFacts } from '@/lib/rpc';
import { clusterNameV1 } from '@/lib/rpcDefault';
import { deadlineMomentPhraseV1, readSlotClockV1, type SlotClockV1 } from '@/lib/slotClock';
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

/**
 * What a market can do, as one line.
 *
 * The chain names capability kinds by content hash, and most of them have no
 * human name yet. A row of `unrecognized kind 2f9cf505bd6a…` chips is a wall of
 * hex where a reader wanted a sentence, so the card counts them instead and
 * names only the ones that have a name. The full per-entry breakdown, hex
 * included, is on the market's own page.
 */
function CapabilityBadges({ capabilities, clock, nowMs }: Readonly<{ capabilities: MarketCapabilityManifestV1 }> & SlotClockPropsV1) {
  if (capabilities.status !== 'authenticated') {
    return <p className="market-capability-refusal">
      <span>{capabilities.status === 'unread' ? 'capabilities unread' : 'capabilities refused'}</span>
      {capabilities.reason}
    </p>;
  }
  const total = capabilities.badges.length;
  if (total === 0) return null;
  const named = capabilities.badges.filter((badge) => badge.recognized);
  const dated = capabilities.badges.filter((badge) => badge.activation === 'deadline' && badge.deadline !== null);
  const soonest = dated.length === 0 ? null : dated.reduce((a, b) => (BigInt(a.deadline ?? '0') <= BigInt(b.deadline ?? '0') ? a : b));
  return <p className="market-capability-summary">
    {total} capability {plural(total, 'entry', 'entries')}
    {named.length > 0 ? ` · ${named.map((badge) => badge.label).join(', ')}` : ''}
    {soonest === null ? '' : ` · ${dated.length === 1 ? 'one' : dated.length} with a deadline${deadlinePhrase(soonest.deadline, clock, nowMs)}`}
  </p>;
}

/**
 * Every listed market's question, in two observations for the whole page.
 *
 * Display-only, so its failure mode is the fallback that was there before:
 * a market whose records did not read keeps its registry row or its address,
 * and one market's refusal costs the others nothing -- which is why the batch
 * reports per market instead of throwing. A page that could not derive any of
 * them still lists every market it read.
 */
async function listedMarketQuestionsV1(
  client: SolanaRpcClient,
  registryProgramId: string | null,
  cards: ReadonlyArray<MarketDiscoveryCardV1>,
): Promise<ReadonlyMap<string, MarketQuestionV1>> {
  const decoded = cards.filter((card): card is Extract<MarketDiscoveryCardV1, { status: 'decoded' }> => card.status === 'decoded');
  if (registryProgramId === null || decoded.length === 0) return new Map();
  try {
    const outcomes = await inspectMarketQuestionsV1(client, {
      registryProgramId,
      markets: decoded.map((card) => Object.freeze({
        address: card.address,
        productRecordId: card.identity.productRecordId,
        resolutionPolicyId: card.identity.resolutionPolicyId,
      })),
    });
    return new Map(outcomes.flatMap((outcome) => outcome.status === 'derived' ? [[outcome.question.address, outcome.question] as const] : []));
  } catch {
    return new Map();
  }
}

/**
 * What each listed market's own records say it asks, keyed by address.
 *
 * A CONTEXT rather than a prop because a card is rendered from six places and
 * threading a map through all six would put the same argument in six
 * signatures for one reader. Empty by default, which is exactly the old
 * behaviour: a card with no derived question falls back to its registry row
 * and then to its address, and every surface that renders a card outside this
 * page keeps working without knowing this exists.
 */
const MarketQuestionsContext = createContext<ReadonlyMap<string, MarketQuestionV1>>(new Map());

function MarketCard({ card, clock, nowMs }: Readonly<{ card: MarketDiscoveryCardV1 }> & SlotClockPropsV1) {
  // Read before the refused early-return below: a hook after a conditional
  // return is a hook that runs in one order for a decoded card and another for
  // a refused one.
  const questions = useContext(MarketQuestionsContext);
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
  // The phase chip keeps saying Open, because that is what the chain says. What
  // the chain does NOT have is a phase for a market whose trading can no longer
  // be switched on, so the card says that in its own words beside the phase
  // rather than editing the phase into something the accounts never claimed.
  const outlook = marketActivationOutlookV1(card);
  const denomination = collateralDenominationV1(card.hoard, card.collateral);
  const narrative = marketNarrativeV1(card.address, card.phase, editorial, questions.get(card.address) ?? null);
  return <article className={`market-discovery-card${outlook.status === 'never' ? ' never-trades' : ''}`}>
    <div className="market-card-top">
      <span className="provenance-chip">{provenanceChipV1(card.provenance)}</span>
      <span className={`phase-chip phase-${card.phase.toLowerCase()}`}>{card.phase}</span>
      {outlook.status === 'never' && <span className="phase-chip never-trades">never trades</span>}
    </div>
    {/* One merge point for what a market is called, shared with the market
        page: the registry where it names one, the market's own records where
        this page has read them, the address last. The read is BATCHED -- two
        observations for the whole list rather than two per card -- which is
        what made deriving here affordable at all. */}
    <h3><Anchor href={marketDetailHrefV1(card.address)} title={card.address}>{narrative.title}</Anchor></h3>
    {narrative.question !== null && <p className="market-question">{narrative.question}</p>}
    <p className="market-card-address" title={card.address}>{shortAddressV1(card.address, 10)}</p>
    {outlook.status === 'never' && <p className="market-never-trades-note">
      Trading can never be switched on. The window closed at slot {outlook.lastActivationSlot}.
    </p>}
    {/* The facts somebody deciding whether to trade asks for. Everything else
        the account carries is one click below. */}
    <dl className="market-card-facts">
      <div><dt>Outcomes</dt><dd>{card.liability.status === 'bound' ? card.liability.claimCount : card.liability.status}</dd></div>
      {/*
        ISSUED, not bought. Every claim on this row was minted by putting
        collateral in for a complete set -- one claim on every outcome at once
        -- which is the opposite of somebody picking a side. Calling the
        founder's own complete sets "claims bought" reads as demand that does
        not exist, and it is the same four numbers on every outcome when it
        happens.
      */}
      <div><dt>Claims issued, per outcome</dt><dd>{card.liability.status === 'bound'
        ? card.liability.supplyAtoms.map((atoms) => formatQuantityV1(atoms, denomination).display).join(' · ')
        : card.liability.status}</dd></div>
      <div><dt>Most it could be asked to pay</dt><dd>{card.liability.status === 'bound' ? card.liability.requiredBackingAtoms : card.liability.status}</dd></div>
      <div><dt>Paid in</dt><dd>{card.collateral.status === 'bound'
        ? <span title={card.collateral.collateralMint}>{card.collateral.collateralMintShort}</span>
        : card.collateral.status}</dd></div>
      <div><dt>Answer decided?</dt><dd>{card.settlement.status === 'terminal'
        ? `yes — ${editorial?.outcomes?.[card.settlement.winner] ?? `outcome ${card.settlement.winner}`} won`
        : 'not yet'}</dd></div>
      {card.hoard.status === 'derived' && <div><dt>Collateral in the vault</dt><dd><strong>{card.hoard.principalAtoms}</strong> atoms</dd></div>}
    </dl>
    <details className="listing-group">
      <summary><span>More fields</span></summary>
      <div>
        <dl className="market-card-facts">
          <div><dt>Generation</dt><dd>{card.generation}</dd></div>
          <div><dt>Founding readiness</dt><dd>{card.readiness}</dd></div>
          <div><dt>Outstanding capabilities</dt><dd>{card.outstandingCapabilities}</dd></div>
          <div><dt>Terminal receipt</dt><dd>{card.settlement.status === 'terminal'
            ? `${card.settlement.label} · winning claim ${card.settlement.winner}`
            : card.settlement.label}</dd></div>
          {card.hoard.status === 'derived' && <div><dt>Vault</dt><dd title={card.hoard.address}>{shortAddressV1(card.hoard.address)}</dd></div>}
          <div><dt>Realm content ID</dt><dd title={card.identity.realmId}>{card.identity.realmId.slice(0, 16)}…</dd></div>
          <div><dt>Finalized observed slot</dt><dd>{card.observedSlot}</dd></div>
        </dl>
        <ul className="market-bindings">
          {card.bindings.map((check) => (
            <li key={check.label} className={check.ok ? 'check-pass' : 'check-fail'}>
              <span aria-hidden="true">{check.ok ? '✓' : '×'}</span>
              <div><strong>{check.label}</strong><small>{check.detail}</small></div>
            </li>
          ))}
        </ul>
      </div>
    </details>
    {/* FE-CHART mount: the issuance split. */}
    {card.liability.status === 'bound' && <SupplyShareStrip
      supplies={card.liability.supplyAtoms}
      outcomes={editorial?.outcomes ?? null}
      caption={SUPPLY_SHARE_MEANING_V1}
      emptyReason="No claims issued yet."
    />}
    {/* FE-CHART mount: the recorded run, for the one market a run recorded. */}
    <MarketIssuanceHistory address={card.address} outcomes={editorial?.outcomes ?? null} />
    {card.hoard.status !== 'derived' && <p className="market-capability-refusal"><span>Vault {card.hoard.status}</span>{card.hoard.reason}</p>}
    <p className="market-observation"><Anchor href={marketDetailHrefV1(card.address)}>See every field on this market →</Anchor></p>
    {card.collateral.status !== 'bound' && <p className="market-refusal">{card.collateral.reason}</p>}
    {card.liability.status !== 'bound' && <p className="market-refusal">{card.liability.reason}</p>}
    <CapabilityBadges capabilities={card.capabilities} clock={clock} nowMs={nowMs} />
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
    title={`${count} older market${plural(count, '', 's')} this build cannot read`}
    note="not listed as current"
  >
    {/*
      The widths are READ, not written. This sentence said "352 bytes where
      this build expects 360" while the current width was 368 and 352 was two
      generations behind: a hand-typed pair of numbers next to a generated
      constant and a maintained list of superseded ones, agreeing with neither.
    */}
    <p className="market-empty">
      Made by an older version of the protocol: {SUPERSEDED_CORE_STATE_WIDTHS.join(' or ')} bytes
      where this build expects {CORE_STATE_BYTES}.
      The explorer will still show you {plural(count, 'its', 'their')} raw bytes.
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
  const untradeable = listing.untradeable.length;
  const unreadable = listing.unreadable.length;
  if (founding + settled + untradeable + unreadable + incompatible.length === 0) return null;
  return <section className="trade-v3-card">
    <header>
      <span>02</span>
      <div><h2>Everything else here</h2><p>Accounts this deployment holds that are not open markets.</p></div>
    </header>

    {untradeable > 0 && <ListingGroup
      title={`${untradeable} market${plural(untradeable, '', 's')} that can never trade`}
      note="trading can no longer be switched on"
    >
      <p className="market-empty">
        Trading has to be switched on within a set window after a market is created, and on
        {plural(untradeable, ' this one', ' these')} the window closed first. Nothing can turn it on now.
        {plural(untradeable, ' Its', ' Their')} claims and collateral are still on the chain.
      </p>
      <div className="market-card-grid">{listing.untradeable.map((card) => <MarketCard key={card.address} card={card} clock={clock} nowMs={nowMs} />)}</div>
    </ListingGroup>}

    {settled > 0 && <ListingGroup
      title={`${settled} market${plural(settled, '', 's')} that already ${plural(settled, 'has', 'have')} an answer`}
      note="resolved, retiring, or retired"
    >
      <div className="market-card-grid">{listing.settled.map((card) => <MarketCard key={card.address} card={card} clock={clock} nowMs={nowMs} />)}</div>
    </ListingGroup>}

    {founding > 0 && <ListingGroup
      title={`${founding} market${plural(founding, '', 's')} that ${plural(founding, 'was', 'were')} never finished`}
      note="setup stopped part-way"
    >
      <p className="market-empty">
        Setting a market up takes a run of transactions, and {plural(founding, 'this one', 'these')} stopped part-way
        through. There is nothing to trade against {plural(founding, 'it', 'them')}.
      </p>
      <div className="market-card-grid">{listing.founding.map((card) => <MarketCard key={card.address} card={card} clock={clock} nowMs={nowMs} />)}</div>
    </ListingGroup>}

    {unreadable > 0 && <ListingGroup
      title={`${unreadable} account${plural(unreadable, '', 's')} we could not read`}
      note="each one says why"
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
        No market on devnet yet.{' '}
        <Anchor href={docsHrefV1('evidence/DEPLOY_1.html', 'docs/evidence/DEPLOY_1.md')}>Read the deployment evidence →</Anchor>
      </p>
      <HistoricalMarketAccounts accounts={incompatible} />
    </div>;
  }
  return <div>
    <p className="market-empty">
      No market on this {deployment.label.toLowerCase()} deployment yet.{' '}
      <Anchor href="/create">Design a market →</Anchor>
    </p>
    <HistoricalMarketAccounts accounts={incompatible} />
  </div>;
}

export default function MarketDiscoveryWorkspace() {
  const deployment = useDeploymentV1();
  const [state, setState] = useState<State>({ kind: 'loading', message: 'Reading the market list…' });
  const discovery = state.kind === 'ready' ? state.discovery : null;
  // The wall-clock layer: a slot-rate clock measured after each read, and a
  // ticking "now". Both start absent, so the server-rendered document and the
  // first client render carry no time strings at all — every estimate appears
  // only once it can actually be estimated.
  const [clock, setClock] = useState<SlotClockV1 | null>(null);
  const [questions, setQuestions] = useState<ReadonlyMap<string, MarketQuestionV1>>(new Map());
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
    setState({ kind: 'loading', message: `Reading every ${deployment.label} market…` });
    setClock(null);
    setQuestions(new Map());
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
      setQuestions(await listedMarketQuestionsV1(client, deployment.programs.registry, next.cards));
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
    untradeable: sortMarketCardsV1(narrowed.untradeable, order),
    settled: sortMarketCardsV1(narrowed.settled, order),
    founding: sortMarketCardsV1(narrowed.founding, order),
    unreadable: sortMarketCardsV1(narrowed.unreadable, order),
  };
  const searching = query.trim().length > 0;
  const incompatible = discovery !== null && discovery.enumeration.mode === 'program-scan'
    ? discovery.enumeration.incompatibleMarketAccounts
    : Object.freeze([]);
  const asideCount = wholeListing === null ? 0 : wholeListing.founding.length + wholeListing.untradeable.length + wholeListing.settled.length + wholeListing.unreadable.length + incompatible.length;

  return <MarketQuestionsContext.Provider value={questions}>
    <PageShell className="product-shell trade-v3-shell" header={<Nav current="/markets" status={`${deployment.label} · read live`} />}>

    <section className="trade-v3-hero hero-solo">
      <div>
        <p className="eyebrow">Markets on {deployment.label}</p>
        <h1>Every market<br /><em>on devnet.</em></h1>
        <p>Markets you can trade come first. Below them: markets whose trading can never be switched on, setups that were never finished, and markets from an older version of the protocol.</p>
      </div>
    </section>

    <section className="trade-v3-card">
      <header>
        <span>01</span>
        <div><h2>Markets you can trade</h2></div>
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
          <article><span>Core program</span><strong>{shortAddressV1(deployment.programs.core, 6)}</strong><small>{deployment.cluster === 'devnet' ? 'the cohort this build names' : 'the active deployment'}</small></article>
        </div>
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
              : <p className="market-empty">Nothing on this deployment has finished founding yet.</p>
            : <div className="market-card-grid">{listing.open.map((card) => <MarketCard key={card.address} card={card} clock={clock} nowMs={nowMs} />)}</div>}
      </>}
    </section>

    {discovery !== null && listing !== null && state.kind === 'ready'
      && <RestOfTheRecord listing={listing} incompatible={incompatible} clock={clock} nowMs={nowMs} />}

  </PageShell>
  </MarketQuestionsContext.Provider>;
}
