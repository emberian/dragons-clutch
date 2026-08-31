'use client';

import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';
import { useCallback, useEffect, useRef, useState } from 'react';

import { useDeploymentV1 } from '@/lib/deploymentStore';

import { type CapabilityFundingQuoteV1 } from '@/lib/capabilityManifest';
import {
  inspectMarketDetailV1,
  requiredBackingMeaningV1,
  type MarketDetailV1,
} from '@/lib/marketDetail';
import { marketEditorialV1 } from '@/lib/marketRegistry';
import {
  marketActivationOutlookV1,
  provenanceChipV1,
  shortAddressV1,
  type MarketActivationOutlookV1,
  type MarketCapabilityBadgeV1,
  type MarketCapabilityManifestV1,
  type MarketCollateralV1,
  type MarketProvenanceV1,
} from '@/lib/marketDiscovery';
import CellStrip from '@/components/charts/CellStrip';
import MarketIssuanceHistory from '@/components/charts/MarketIssuanceHistory';
import SupplyShareStrip from '@/components/charts/SupplyShareStrip';
import { SUPPLY_SHARE_MEANING_V1 } from '@/lib/supplyShares';
import AggregateRetirementStatus from '@/components/AggregateRetirementStatus';
import JoinPanel from '@/components/JoinPanel';
import MarketTradePanel from '@/components/MarketTradePanel';
import RefusedMarketStory from '@/components/RefusedMarketStory';
import { SolanaRpcClient, type ConnectionFacts } from '@/lib/rpc';
import { clusterNameV1 } from '@/lib/rpcDefault';
import { deadlineMomentPhraseV1, readSlotClockV1, slotClockCaveatV1, type SlotClockV1 } from '@/lib/slotClock';
import { watchSentenceV1 } from '@/lib/rpcSubscribe';
import { useAccountWatchV1 } from '@/lib/useAccountWatch';

type State =
  | Readonly<{ kind: 'idle' | 'loading' | 'refused'; message: string }>
  | Readonly<{ kind: 'ready'; message: string; detail: MarketDetailV1; facts: ConnectionFacts }>;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'the detail read refused without a usable reason';
}

/** Every section states where its own bytes came from, or why it has none. */
function SectionProvenance({ provenance }: Readonly<{ provenance: MarketProvenanceV1 }>) {
  return <div className="detail-section-provenance">
    <span className={`provenance-chip${provenance.kind === 'refused' ? ' refused' : ''}`}>{provenanceChipV1(provenance)}</span>
    {provenance.kind === 'refused' && <small>{provenance.reason}</small>}
  </div>;
}

function Fact({ label, value, title }: Readonly<{ label: string; value: string; title?: string }>) {
  return <div><dt>{label}</dt><dd title={title ?? value}>{value}</dd></div>;
}

function ContentId({ label, value }: Readonly<{ label: string; value: string }>) {
  return <div className="detail-identity">
    <dt>{label}</dt>
    <dd><code title={value}>{value}</code></dd>
  </div>;
}

function CopyableAddress({ label, address }: Readonly<{ label: string; address: string }>) {
  const [copied, setCopied] = useState(false);
  return <div className="detail-copyable">
    <dt>{label}</dt>
    <dd>
      <span title={address}>{shortAddressV1(address, 6)}</span>
      <code>{address}</code>
      <button
        type="button"
        className="secondary-action"
        onClick={() => { void navigator.clipboard?.writeText(address).then(() => setCopied(true)).catch(() => setCopied(false)); }}
      >{copied ? 'copied' : 'copy full address'}</button>
    </dd>
  </div>;
}

/**
 * The seven segregated compartments, each with its own asset class. Native
 * lamports and Realm collateral atoms are two physical dimensions and are never
 * added together, here or anywhere else.
 */
function FundingQuote({ funding }: Readonly<{ funding: CapabilityFundingQuoteV1 }>) {
  return <div className="capability-funding">
    <table>
      <thead><tr><th>Compartment</th><th>What kind of asset</th><th>Amount · raw</th></tr></thead>
      <tbody>
        {funding.compartments.map((compartment) => (
          <tr key={compartment.compartment} className={compartment.assetClass === 'not-applicable' ? 'compartment-empty' : ''}>
            <td>{compartment.compartment}</td>
            <td>{compartment.assetClass}</td>
            <td>{compartment.amount.toString()}</td>
          </tr>
        ))}
      </tbody>
      <tfoot>
        <tr><td>Total in SOL</td><td>native-lamports</td><td>{funding.nativeLamportsTotal.toString()}</td></tr>
        <tr><td>Total in collateral</td><td>realm-collateral</td><td>{funding.realmCollateralTotal.toString()}</td></tr>
      </tfoot>
    </table>
    {funding.realmCollateral === null
      ? <p>Costs no collateral.</p>
      : <dl className="detail-facts">
        <Fact label="Bound collateral mint" value={funding.realmCollateral.mint.reduce((text, byte) => text + byte.toString(16).padStart(2, '0'), '')} />
        <Fact label="Bound token program" value={funding.realmCollateral.tokenProgram.reduce((text, byte) => text + byte.toString(16).padStart(2, '0'), '')} />
      </dl>}
  </div>;
}

type SlotClockPropsV1 = Readonly<{ clock?: SlotClockV1 | null; nowMs?: number | null }>;

function deadlinePhrase(deadline: string | null, clock: SlotClockV1 | null | undefined, nowMs: number | null | undefined): string {
  if (deadline === null || clock === undefined || clock === null || nowMs === undefined || nowMs === null) return '';
  return ` · ${deadlineMomentPhraseV1(clock, deadline, nowMs)}`;
}

function CapabilityEntry({ badge, clock, nowMs }: Readonly<{ badge: MarketCapabilityBadgeV1 }> & SlotClockPropsV1) {
  return <details className="capability-drawer">
    <summary>
      <span className={`capability-badge${badge.recognized ? ' recognized' : ''}`}>{badge.recognized ? badge.label : `Capability entry ${badge.index}`}</span>
      <small>{badge.activation === 'deadline' ? `switches on by slot ${badge.deadline}${deadlinePhrase(badge.deadline, clock, nowMs)}` : 'switches on immediately'}</small>
    </summary>
    <dl className="detail-facts">
      <ContentId label="What kind it is" value={badge.kindId} />
      <ContentId label="Which release runs it" value={badge.programSetId} />
      <ContentId label="How it is configured" value={badge.configId} />
      <Fact label="When it switches on" value={badge.activation} />
      <Fact label="Must be switched on by" value={badge.deadline === null ? 'no deadline — it switches on the moment it is asked to' : `slot ${badge.deadline}${deadlinePhrase(badge.deadline, clock, nowMs)}`} />
      <Fact label="Waits for" value={badge.dependencies.length === 0 ? 'nothing' : badge.dependencies.join(', ')} />
    </dl>
    <FundingQuote funding={badge.funding} />
  </details>;
}

function Capabilities({ capabilities, clock, nowMs }: Readonly<{ capabilities: MarketCapabilityManifestV1 }> & SlotClockPropsV1) {
  if (capabilities.status !== 'authenticated') {
    return <p className="market-capability-refusal">
      <span>{capabilities.status === 'unread' ? 'capabilities unread' : 'capabilities refused'}</span>
      {capabilities.reason}
    </p>;
  }
  return <>
    <dl className="detail-facts">
      <ContentId label="Fingerprint of the list" value={capabilities.manifestId} />
      <Fact label="Where the list is stored" value={capabilities.recordAddress} />
      <Fact label="Things it can do" value={String(capabilities.badges.length)} />
    </dl>
    <div className="capability-drawers">{capabilities.badges.map((badge) => <CapabilityEntry key={badge.index} badge={badge} clock={clock} nowMs={nowMs} />)}</div>
  </>;
}

function Realm({ collateral }: Readonly<{ collateral: MarketCollateralV1 }>) {
  if (collateral.status !== 'bound') {
    return <p className="market-refusal">{collateral.reason}</p>;
  }
  return <dl className="detail-facts">
    <Fact label="Collateral setup account" value={collateral.realmAddress} />
    <ContentId label="Its fingerprint" value={collateral.realmContentId} />
    <Fact label="Token program" value={collateral.tokenProgram} />
    <CopyableAddress label="The token it pays out in" address={collateral.collateralMint} />
    <ContentId label="Release that handles that token" value={collateral.adapterReleaseId} />
    <Fact label="Who may mint more of it" value={collateral.mintAuthorityPolicy} />
    <Fact label="Who may freeze it" value={collateral.freezeAuthorityPolicy} />
  </dl>;
}

export default function MarketDetailWorkspace({ address }: Readonly<{ address: string }>) {
  const deployment = useDeploymentV1();
  // Editorial words for this address, if the shipped registry has any. They
  // never gate a read and never stand in for one: an unregistered market
  // renders its address, exactly as before.
  const editorial = marketEditorialV1(address);
  const [state, setState] = useState<State>({ kind: 'loading', message: 'Reading this market…' });
  const detail = state.kind === 'ready' ? state.detail : null;
  const card = detail?.card ?? null;
  const decoded = card !== null && card.status === 'decoded' ? card : null;
  const refused = card !== null && card.status === 'refused' ? card : null;
  // Wall-clock layer: measured slot-rate clock plus a ticking now, both
  // absent until they can be true — see MarketDiscoveryWorkspace.
  const [clock, setClock] = useState<SlotClockV1 | null>(null);
  const [nowMs, setNowMs] = useState<number | null>(null);

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

  const read = useCallback(async () => {
    setState({ kind: 'loading', message: 'Reading this market…' });
    setClock(null);
    try {
      const client = new SolanaRpcClient(deployment.endpoint);
      const facts = await client.probe();
      const next = await inspectMarketDetailV1(client, {
        coreProgramId: deployment.programs.core,
        registryProgramId: deployment.programs.registry,
        claimsProgramId: deployment.programs.claims,
        custodyProgramId: deployment.programs.custody,
        address,
      });
      setState({ kind: 'ready', detail: next, facts, message: next.reason });
      setClock(await readSlotClockV1(client, next.floorSlot));
    } catch (error) {
      setState({ kind: 'refused', message: `Refused: ${errorMessage(error)}` });
    }
  }, [address, deployment]);

  // Content on load: the address is in the URL and the deployment is baked,
  // so there is nothing left to ask for before reading the chain.
  useEffect(() => {
    let cancelled = false;
    queueMicrotask(() => {
      if (!cancelled) void read();
    });
    return () => {
      cancelled = true;
    };
  }, [read]);

  // The live layer. Two accounts carry everything that can change under a
  // reader on this page — the Market root and the Claims aggregate holding its
  // liabilities — so those are the only two watched, and a market whose
  // aggregate was not read watches only the root.
  //
  // A notification is never decoded here. It says "what you read is stale",
  // and the answer is to run the SAME bounded finalized read the page already
  // uses, so nothing on screen can come from a second, unaudited path. The
  // re-read is delayed a moment because a transaction usually moves both
  // accounts and a reader does not need two reads for one event.
  const watched = decoded === null
    ? [address]
    : decoded.liability.status === 'bound'
      ? [address, decoded.liability.aggregateAddress]
      : [address];
  const [changedAtSlot, setChangedAtSlot] = useState<string | null>(null);
  const reread = useRef<ReturnType<typeof setTimeout> | null>(null);
  const watchState = useAccountWatchV1(deployment.endpoint, watched, (change) => {
    setChangedAtSlot(change.slot);
    if (reread.current !== null) clearTimeout(reread.current);
    reread.current = setTimeout(() => { void read(); }, 1_200);
  });
  useEffect(() => () => {
    if (reread.current !== null) clearTimeout(reread.current);
  }, []);

  const marketProvenance: MarketProvenanceV1 = card?.provenance
    ?? Object.freeze({ kind: 'refused', reason: 'This market has not been read from the chain yet.' });
  const realmProvenance = detail?.realmProvenance
    ?? Object.freeze({ kind: 'refused' as const, reason: 'Not read yet — the market itself has not been read.' });
  const liabilityProvenance = detail?.liabilityProvenance
    ?? Object.freeze({ kind: 'refused' as const, reason: 'Not read yet — the market itself has not been read.' });
  const capabilityProvenance = detail?.capabilityProvenance
    ?? Object.freeze({ kind: 'refused' as const, reason: 'Not read yet — the market itself has not been read.' });
  const activation: MarketActivationOutlookV1 = card === null
    ? Object.freeze({ status: 'unknown', reason: 'This market has not been read from the chain yet.' })
    : marketActivationOutlookV1(card);

  return <main className="product-shell trade-v3-shell">
    <Nav current="/markets" status={`${deployment.label} · read live`} />

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow"><Anchor href="/markets">← all markets</Anchor> · one market, field by field</p>
        <h1>{editorial === null ? shortAddressV1(address, 8) : editorial.title}<br /><em>{decoded === null ? (state.kind === 'loading' ? 'reading…' : card !== null && card.status === 'refused' ? 'refused' : 'unread') : decoded.phase}</em></h1>
        {editorial !== null && <p className="market-question">{editorial.question}</p>}
        {editorial !== null && editorial.resolution !== null && <p className="market-resolution">{editorial.resolution}</p>}
        {editorial !== null && editorial.story !== null && <p className="market-story">{editorial.story}</p>}
      </div>
      <aside>
        <span>Address</span>
        <strong>{shortAddressV1(address, 10)}</strong>
        <p><code>{address}</code></p>
        <p><Anchor href={`/explorer?view=market&q=${encodeURIComponent(address)}`}>
          See everything it is connected to →
        </Anchor></p>
      </aside>
    </section>

    <section className="trade-v3-card route-card">
      <header>
        <span>00</span>
        <div><h2>Connection</h2><p>Reading {deployment.label}.</p></div>
        <div className="direct-actions"><button type="button" onClick={() => void read()} disabled={state.kind === 'loading'}>{state.kind === 'loading' ? 'Reading…' : 'Read it again'}</button></div>
      </header>
      <p className="direct-status" aria-live="polite">{state.message}</p>
      {state.kind === 'ready' && <div className="trade-v3-evidence">
        <article><span>Endpoint</span><strong>{state.facts.solanaCore}</strong><small>{clusterNameV1(state.facts.genesisHash)} · genesis {shortAddressV1(state.facts.genesisHash, 6)}</small></article>
        <article><span>Finalized floor</span><strong>{state.detail.floorSlot}</strong><small>{clock === null ? 'slot' : `read at ${new Date(clock.observedAtMs).toLocaleTimeString()}`}</small></article>
        <article><span>Core program</span><strong>{shortAddressV1(state.detail.coreProgramId, 6)}</strong><small>owner of this account</small></article>
        <article><span>Registry program</span><strong>{state.detail.registryProgramId === null ? 'not selected' : shortAddressV1(state.detail.registryProgramId, 6)}</strong><small>{state.detail.registryProgramId === null ? 'not read' : 'authenticated'}</small></article>
      </div>}
      {clock !== null && <p className="slot-clock-note">{slotClockCaveatV1(clock)}</p>}
      {/* The live layer, stated rather than implied. A reader is told whether
          this page is watching, and an endpoint that cannot carry a
          subscription is a fact about the connection — never about the
          market, and never a reason to distrust what is already on screen. */}
      <p className={watchState === 'unavailable' ? 'market-capability-refusal' : 'live-watch-note'} aria-live="polite">
        {watchState === 'unavailable' && <span>not watching</span>}
        {watchState === 'live' && <i className="live-watch-dot" />}
        {watchSentenceV1(watchState, deployment.label)}
        {changedAtSlot !== null && watchState === 'live'
          ? ` It last changed at slot ${changedAtSlot}, and this page re-read it.`
          : ''}
      </p>
    </section>

    <section className="trade-v3-card">
      <header><span>01</span><div><h2>What this market is</h2><p>What it locked itself to when it was created. None of it can change.</p></div><SectionProvenance provenance={marketProvenance} /></header>
      {refused !== null && <RefusedMarketStory refusal={refused.refusal} observedSlot={refused.observedSlot} address={address} />}
      {decoded === null
        ? refused === null && <p className="market-empty">Not read yet.</p>
        : <>
          <dl className="detail-facts">
            <CopyableAddress label="Market address" address={decoded.address} />
            <Fact label="Phase" value={decoded.phase} />
            <Fact label="Read at finalized slot" value={decoded.observedSlot} />
            <Fact label="Schema" value={`${decoded.identity.schemaMagic} · version ${decoded.identity.schemaVersion}`} />
            <Fact label="Account width" value={`${decoded.identity.accountBytes} bytes, exact`} />
            <Fact label="Founding readiness" value={decoded.readiness} />
            <Fact label="Generation" value={decoded.generation} />
            <Fact label="Outstanding capabilities" value={decoded.outstandingCapabilities} />
            <Fact label="Permissions checked against" value={decoded.identity.registryProgram} />
            <Fact label="Rent goes back to" value={decoded.identity.rentBeneficiary} />
          </dl>
          <p className="phase-meaning"><strong>{decoded.phase}</strong> {detail?.phaseMeaning}</p>
          {/* The chain has no phase for a market whose trading can never be
              switched on, so the page says it beside the phase rather than
              leaving a reader to infer it from an elapsed deadline. */}
          {activation.status === 'never' && <p className="market-never-trades-note">
            Trading can never be switched on. The window closed at slot {activation.lastActivationSlot}.
          </p>}
          <h3 className="detail-subhead">What it locked itself to</h3>
          <dl className="detail-facts">
            <ContentId label="Its collateral setup" value={decoded.identity.realmId} />
            <ContentId label="The kind of market it is" value={decoded.identity.productRecordId} />
            <ContentId label="This particular market" value={decoded.identity.productInstanceId} />
            <ContentId label="How it gets its answer" value={decoded.identity.resolutionPolicyId} />
            <ContentId label="What it is allowed to do" value={decoded.identity.capabilityManifestId} />
            <ContentId label="Which release runs it" value={decoded.identity.selectedReleaseSetId} />
          </dl>
          <ul className="market-bindings">
            {decoded.bindings.map((check) => (
              <li key={check.label} className={check.ok ? 'check-pass' : 'check-fail'}>
                <span aria-hidden="true">{check.ok ? '✓' : '×'}</span>
                <div><strong>{check.label}</strong><small>{check.detail}</small></div>
              </li>
            ))}
          </ul>
        </>}
    </section>

    {refused === null && <section className="trade-v3-card">
      <header><span>02</span><div><h2>The money</h2></div><SectionProvenance provenance={liabilityProvenance} /></header>
      {decoded === null
        ? <p className="market-empty">Not read yet.</p>
        : decoded.liability.status !== 'bound'
          ? <p className="market-capability-refusal"><span>{decoded.liability.status === 'unread' ? 'liabilities unread' : 'liabilities refused'}</span>{decoded.liability.reason}</p>
          : <>
            <div className="trade-v3-preview">
              <div><span>Collateral it must hold</span><strong>{decoded.liability.requiredBackingAtoms}</strong></div>
              <div><span>Outcomes</span><strong>{decoded.liability.claimCount}</strong></div>
              <div><span>Ledger revision</span><strong>{decoded.liability.revision}</strong></div>
              <div><span>Answer decided?</span><strong>{decoded.settlement.status === 'terminal' ? 'yes' : 'not yet'}</strong></div>
              <p>{requiredBackingMeaningV1(decoded.liability.requiredBackingBasis)}</p>
            </div>
            <dl className="detail-facts">
              <Fact label="Claims ledger account" value={decoded.liability.aggregateAddress} />
              <Fact label="Claims program" value={decoded.liability.claimsProgramId} />
              <ContentId label="Rule it pays by" value={decoded.liability.liabilityBasisId} />
            </dl>
            <h3 className="detail-subhead">Claims bought, per outcome</h3>
            {/* FE-CHART mount: the cell strip draws the same aggregate the
                list below itemizes; the list stays as the exact-value twin. */}
            <CellStrip
              supplies={decoded.liability.supplyAtoms}
              winner={decoded.settlement.status === 'terminal' ? decoded.settlement.winner : null}
              requiredBackingAtoms={decoded.liability.requiredBackingAtoms}
              requiredBackingNote={requiredBackingMeaningV1(decoded.liability.requiredBackingBasis)}
              caption="Claims issued per outcome, against what this market must be able to pay."
              notes={decoded.liability.supplyAtoms.map((_, index) => {
                const outcome = editorial?.outcomes?.[index];
                const status = decoded.settlement.status === 'terminal'
                  ? (decoded.settlement.winner === index ? 'won' : 'lost · pays nothing')
                  : 'no answer yet';
                return outcome === undefined ? status : `${outcome} · ${status}`;
              })}
            />
            <ol className="outcome-vector">
              {decoded.liability.supplyAtoms.map((amount, index) => {
                const outcomeName = editorial !== null && editorial.outcomes !== null ? editorial.outcomes[index] : undefined;
                return (
                <li key={index} className={decoded.settlement.status === 'terminal' && decoded.settlement.winner === index ? 'winning-outcome' : ''}>
                  <span>claim {index}{outcomeName === undefined ? '' : ` · ${outcomeName}`}</span>
                  <strong>{amount}</strong>
                  {decoded.settlement.status === 'terminal' && <small>{decoded.settlement.winner === index ? 'won' : 'lost · pays nothing'}</small>}
                </li>
                );
              })}
            </ol>
            <h3 className="detail-subhead">How those claims are split</h3>
            {/* FE-CHART mount: the same supply vector as the cell strip and
                the ordered list, re-expressed as shares of the whole. */}
            <SupplyShareStrip
              supplies={decoded.liability.supplyAtoms}
              outcomes={editorial?.outcomes ?? null}
              caption={SUPPLY_SHARE_MEANING_V1}
              emptyReason="No claims issued yet."
            />
            {/* Drawn only for a market some run actually recorded; every other
                market renders nothing here rather than an empty frame. */}
            <MarketIssuanceHistory address={address} outcomes={editorial?.outcomes ?? null} />
            <h3 className="detail-subhead">The vault</h3>
            {decoded.hoard.status === 'derived'
              ? <dl className="detail-facts">
                <Fact label="Collateral held (raw)" value={decoded.hoard.principalAtoms} />
                <Fact label="Vault account" value={decoded.hoard.address} />
                <Fact label="Only this may move it" value={decoded.hoard.custodyAuthority} />
                <ContentId label="Under this custody namespace" value={decoded.hoard.custodyContext} />
                <Fact label="Custody program" value={decoded.hoard.custodyProgramId} />
                <Fact label="Read at finalized slot" value={decoded.hoard.observedSlot} />
              </dl>
              : <p className="market-capability-refusal"><span>Vault {decoded.hoard.status}</span>{decoded.hoard.reason}</p>}
            <h3 className="detail-subhead">The answer</h3>
            {decoded.settlement.status === 'terminal'
              ? <dl className="detail-facts">
                <Fact label="State" value={decoded.settlement.label} />
                <Fact label="Outcome that won" value={String(decoded.settlement.winner)} />
                <ContentId label="Fingerprint of the answer" value={decoded.settlement.receiptId} />
              </dl>
              : <p className="market-hoard-note">No answer recorded yet.</p>}
          </>}
    </section>}

    {refused === null && <section className="trade-v3-card">
      <header><span>03</span><div><h2>What it pays out in</h2><p>The protocol calls this the Realm.</p></div><SectionProvenance provenance={realmProvenance} /></header>
      {decoded === null
        ? <p className="market-empty">Not read yet.</p>
        : <Realm collateral={decoded.collateral} />}
    </section>}

    {refused === null && <section className="trade-v3-card">
      <header><span>04</span><div><h2>What it is allowed to do</h2></div><SectionProvenance provenance={capabilityProvenance} /></header>
      {decoded === null
        ? <p className="market-empty">Not read yet.</p>
        : <Capabilities capabilities={decoded.capabilities} clock={clock} nowMs={nowMs} />}
    </section>}

    {decoded !== null && <JoinPanel
      endpoint={deployment.endpoint}
      marketAddress={address}
      marketPhase={decoded.phase}
      coreProgramId={deployment.programs.core}
      registryProgramId={deployment.programs.registry}
      claimsProgramId={deployment.programs.claims}
      tradingProgramId={deployment.programs.trading}
      custodyProgramId={deployment.programs.custody}
      rentProgramId={deployment.programs.rent}
    />}

    {decoded !== null && <MarketTradePanel
      endpoint={deployment.endpoint}
      marketAddress={address}
      coreProgramId={deployment.programs.core}
      registryProgramId={deployment.programs.registry}
      claimsProgramId={deployment.programs.claims}
      tradingProgramId={deployment.programs.trading}
      custodyProgramId={deployment.programs.custody}
      rentProgramId={deployment.programs.rent}
      liability={decoded.liability}
    />}

    {decoded !== null && <AggregateRetirementStatus
      endpoint={deployment.endpoint}
      coreProgramId={deployment.programs.core}
      claimsProgramId={deployment.programs.claims}
      marketAddress={address}
      marketPhase={decoded.phase}
      marketGeneration={decoded.generation}
      minimumContextSlot={state.kind === 'ready' ? state.detail.floorSlot : decoded.observedSlot}
    />}

  </main>;
}
