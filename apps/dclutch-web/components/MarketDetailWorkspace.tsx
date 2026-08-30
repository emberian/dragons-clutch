'use client';

import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';
import { useCallback, useEffect, useState } from 'react';

import { useDeploymentV1 } from '@/lib/deploymentStore';

import { type CapabilityFundingQuoteV1 } from '@/lib/capabilityManifest';
import {
  inspectMarketDetailV1,
  requiredBackingMeaningV1,
  type MarketDetailV1,
} from '@/lib/marketDetail';
import { MARKET_EDITORIAL_NOTE_V1, marketEditorialV1 } from '@/lib/marketRegistry';
import {
  provenanceChipV1,
  shortAddressV1,
  type MarketCapabilityBadgeV1,
  type MarketCapabilityManifestV1,
  type MarketCollateralV1,
  type MarketProvenanceV1,
} from '@/lib/marketDiscovery';
import CellStrip from '@/components/charts/CellStrip';
import AggregateRetirementStatus from '@/components/AggregateRetirementStatus';
import JoinPanel from '@/components/JoinPanel';
import MarketTradePanel from '@/components/MarketTradePanel';
import RefusedMarketStory from '@/components/RefusedMarketStory';
import { SolanaRpcClient, type ConnectionFacts } from '@/lib/rpc';
import { clusterNameV1 } from '@/lib/rpcDefault';
import { deadlineMomentPhraseV1, readSlotClockV1, slotClockCaveatV1, type SlotClockV1 } from '@/lib/slotClock';

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
      <thead><tr><th>Compartment</th><th>Asset class</th><th>Amount · raw u64</th></tr></thead>
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
        <tr><td>Native lamport total</td><td>native-lamports</td><td>{funding.nativeLamportsTotal.toString()}</td></tr>
        <tr><td>Realm collateral total</td><td>realm-collateral</td><td>{funding.realmCollateralTotal.toString()}</td></tr>
      </tfoot>
    </table>
    {funding.realmCollateral === null
      ? <p>This capability quotes no Realm collateral, so it carries no collateral binding.</p>
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
      <span className={`capability-badge${badge.recognized ? ' recognized' : ''}`}>{badge.label}</span>
      <small>entry {badge.index} · {badge.activation === 'deadline' ? `activation deadline slot ${badge.deadline}${deadlinePhrase(badge.deadline, clock, nowMs)}` : 'immediate activation'}</small>
    </summary>
    <dl className="detail-facts">
      <ContentId label="Kind ID" value={badge.kindId} />
      <ContentId label="Release / program-set ID" value={badge.programSetId} />
      <ContentId label="Config ID" value={badge.configId} />
      <Fact label="Activation policy" value={badge.activation} />
      <Fact label="Activation deadline slot" value={badge.deadline === null ? 'none — activation is immediate' : `${badge.deadline}${deadlinePhrase(badge.deadline, clock, nowMs)}`} />
      <Fact label="Depends on entries" value={badge.dependencies.length === 0 ? 'none' : badge.dependencies.join(', ')} />
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
      <ContentId label="Manifest content ID" value={capabilities.manifestId} />
      <Fact label="Registry record" value={capabilities.recordAddress} />
      <Fact label="Entries" value={String(capabilities.badges.length)} />
    </dl>
    <div className="capability-drawers">{capabilities.badges.map((badge) => <CapabilityEntry key={badge.index} badge={badge} clock={clock} nowMs={nowMs} />)}</div>
  </>;
}

function Realm({ collateral }: Readonly<{ collateral: MarketCollateralV1 }>) {
  if (collateral.status !== 'bound') {
    return <p className="market-refusal">{collateral.reason}</p>;
  }
  return <dl className="detail-facts">
    <Fact label="Realm account" value={collateral.realmAddress} />
    <ContentId label="Realm content ID" value={collateral.realmContentId} />
    <Fact label="Token program" value={collateral.tokenProgram} />
    <CopyableAddress label="Collateral mint" address={collateral.collateralMint} />
    <ContentId label="Collateral adapter release ID" value={collateral.adapterReleaseId} />
    <Fact label="Mint authority policy" value={collateral.mintAuthorityPolicy} />
    <Fact label="Freeze authority policy" value={collateral.freezeAuthorityPolicy} />
  </dl>;
}

export default function MarketDetailWorkspace({ address }: Readonly<{ address: string }>) {
  const deployment = useDeploymentV1();
  // Editorial words for this address, if the shipped registry has any. They
  // never gate a read and never stand in for one: an unregistered market
  // renders its address, exactly as before.
  const editorial = marketEditorialV1(address);
  const [state, setState] = useState<State>({ kind: 'loading', message: 'Reading this Market at the finalized floor…' });
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
    setState({ kind: 'loading', message: 'Reading this Market, the Realm record and capability manifest it commits to, and the Claims aggregate holding its liabilities, behind one finalized floor…' });
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

  const marketProvenance: MarketProvenanceV1 = card?.provenance
    ?? Object.freeze({ kind: 'refused', reason: 'This Market has not been read at any finalized floor yet.' });
  const realmProvenance = detail?.realmProvenance
    ?? Object.freeze({ kind: 'refused' as const, reason: 'No Realm has been reacquired, because no Market has been read.' });
  const liabilityProvenance = detail?.liabilityProvenance
    ?? Object.freeze({ kind: 'refused' as const, reason: 'No Claims aggregate has been read, because no Market has been read.' });
  const capabilityProvenance = detail?.capabilityProvenance
    ?? Object.freeze({ kind: 'refused' as const, reason: 'Nothing to show yet: no market has been read, so there is nothing to check.' });

  return <main className="product-shell trade-v3-shell">
    <Nav current="/markets" status={`${deployment.label} · finalized reads`} />

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow"><Anchor href="/markets">← all Markets</Anchor> · one Market, decoded field by field</p>
        <h1>{editorial === null ? shortAddressV1(address, 8) : editorial.title}<br /><em>{decoded === null ? (state.kind === 'loading' ? 'reading…' : card !== null && card.status === 'refused' ? 'refused' : 'unread') : decoded.phase}</em></h1>
        {editorial !== null && <p className="market-question">{editorial.question}</p>}
        {editorial !== null && editorial.story !== null && <p className="market-story">{editorial.story}</p>}
        {editorial !== null && <p className="market-editorial-note">{MARKET_EDITORIAL_NOTE_V1}</p>}
        <p>Every field below is decoded from a finalized account this browser read, or the section carries REFUSED and its exact reason. Nothing here is aggregated, estimated, or carried over from a previous observation, and no sub-state renders as empty-but-fine.</p>
      </div>
      <aside>
        <span>Market address</span>
        <strong>{shortAddressV1(address, 10)}</strong>
        <p><code>{address}</code></p>
        <p><Anchor href={`/explorer?view=market&q=${encodeURIComponent(address)}`}>
          Open the record graph in the explorer →
        </Anchor></p>
      </aside>
    </section>

    <section className="trade-v3-card route-card">
      <header>
        <span>00</span>
        <div><h2>The read</h2><p>The program that owns this account is what decides what its bytes mean, and it — like every other program named on this page — comes from the active {deployment.label} deployment. What this market is allowed to do is checked against that same deployment, not taken from the market&apos;s own word for it.</p></div>
        <div className="direct-actions"><button type="button" onClick={() => void read()} disabled={state.kind === 'loading'}>{state.kind === 'loading' ? 'Reading…' : 'Re-read this Market'}</button></div>
      </header>
      <p className="direct-status" aria-live="polite">{state.message}</p>
      {state.kind === 'ready' && <div className="trade-v3-evidence">
        <article><span>Endpoint</span><strong>{state.facts.solanaCore}</strong><small>{clusterNameV1(state.facts.genesisHash)} · genesis {shortAddressV1(state.facts.genesisHash, 6)}</small></article>
        <article><span>Finalized floor</span><strong>{state.detail.floorSlot}</strong><small>{clock === null ? 'one observation epoch for every section' : `read at ${new Date(clock.observedAtMs).toLocaleTimeString()} · one observation epoch for every section`}</small></article>
        <article><span>Core program</span><strong>{shortAddressV1(state.detail.coreProgramId, 6)}</strong><small>owner of these bytes</small></article>
        <article><span>Capability source</span><strong>{state.detail.registryProgramId === null ? 'not selected' : shortAddressV1(state.detail.registryProgramId, 6)}</strong><small>{state.detail.registryProgramId === null ? 'manifest unread' : 'manifest authenticated by content'}</small></article>
      </div>}
      {clock !== null && <p className="slot-clock-note">{slotClockCaveatV1(clock)}</p>}
    </section>

    <section className="trade-v3-card">
      <header><span>01</span><div><h2>Overview</h2><p>What this account is, and the immutable identities it committed to when it was founded.</p></div><SectionProvenance provenance={marketProvenance} /></header>
      {refused !== null && <RefusedMarketStory refusal={refused.refusal} observedSlot={refused.observedSlot} address={address} />}
      {decoded === null
        ? refused === null && <p className="market-empty">No decoded Market root. Nothing about phase, generation, or identity is asserted until one finalized read succeeds.</p>
        : <>
          <dl className="detail-facts">
            <CopyableAddress label="Market address" address={decoded.address} />
            <Fact label="Schema" value={`${decoded.identity.schemaMagic} · version ${decoded.identity.schemaVersion}`} />
            <Fact label="Account width" value={`${decoded.identity.accountBytes} bytes, exact`} />
            <Fact label="Phase" value={decoded.phase} />
            <Fact label="Founding readiness" value={decoded.readiness} />
            <Fact label="Generation" value={decoded.generation} />
            <Fact label="Outstanding capabilities" value={decoded.outstandingCapabilities} />
            <Fact label="Finalized observed slot" value={decoded.observedSlot} />
            <Fact label="Selected Registry program" value={decoded.identity.registryProgram} />
            <Fact label="Rent beneficiary" value={decoded.identity.rentBeneficiary} />
          </dl>
          <p className="phase-meaning"><strong>{decoded.phase}</strong> {detail?.phaseMeaning}</p>
          <h3 className="detail-subhead">Immutable identities · content IDs, not addresses</h3>
          <dl className="detail-facts">
            <ContentId label="Realm" value={decoded.identity.realmId} />
            <ContentId label="Product record" value={decoded.identity.productRecordId} />
            <ContentId label="Product instance" value={decoded.identity.productInstanceId} />
            <ContentId label="Resolution policy" value={decoded.identity.resolutionPolicyId} />
            <ContentId label="Capability manifest" value={decoded.identity.capabilityManifestId} />
            <ContentId label="Selected execution release set" value={decoded.identity.selectedReleaseSetId} />
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
      <header><span>02</span><div><h2>Economics</h2><p>Raw u64 atoms, read where the chain keeps them. A Core Market root carries no supply vector and no Hoard figure, so this section is not decoded from the Market at all: the per-claim supplies come from the Claims LiabilityBasisV2 aggregate this Market derives, and the Hoard is stated as underivable rather than guessed.</p></div><SectionProvenance provenance={liabilityProvenance} /></header>
      {decoded === null
        ? <p className="market-empty">No decoded economic state. A zero is a fact a read has to justify, so none is shown here.</p>
        : decoded.liability.status !== 'bound'
          ? <p className="market-capability-refusal"><span>{decoded.liability.status === 'unread' ? 'liabilities unread' : 'liabilities refused'}</span>{decoded.liability.reason}</p>
          : <>
            <div className="trade-v3-preview">
              <div><span>Exact required backing</span><strong>{decoded.liability.requiredBackingAtoms}</strong></div>
              <div><span>Claim count</span><strong>{decoded.liability.claimCount}</strong></div>
              <div><span>Aggregate revision</span><strong>{decoded.liability.revision}</strong></div>
              <div><span>Terminal receipt</span><strong>{decoded.settlement.status === 'terminal' ? 'accepted' : 'none'}</strong></div>
              <p>Required backing is measured against the {requiredBackingMeaningV1(decoded.liability.requiredBackingBasis)}.</p>
            </div>
            <dl className="detail-facts">
              <Fact label="Claims aggregate account" value={decoded.liability.aggregateAddress} />
              <Fact label="Claims program" value={decoded.liability.claimsProgramId} />
              <ContentId label="Liability basis" value={decoded.liability.liabilityBasisId} />
            </dl>
            <h3 className="detail-subhead">Per-claim supply · ordered, raw u64</h3>
            {/* FE-CHART mount: the cell strip draws the same aggregate the
                list below itemizes; the list stays as the exact-value twin. */}
            <CellStrip
              supplies={decoded.liability.supplyAtoms}
              winner={decoded.settlement.status === 'terminal' ? decoded.settlement.winner : null}
              requiredBackingAtoms={decoded.liability.requiredBackingAtoms}
              requiredBackingNote={requiredBackingMeaningV1(decoded.liability.requiredBackingBasis)}
              caption="Each cell is one claim across this Market&rsquo;s outcome domain; heights are issued claim atoms from the Claims aggregate, against the exact required-backing line."
              notes={decoded.liability.supplyAtoms.map((_, index) => {
                const outcome = editorial?.outcomes?.[index];
                const status = decoded.settlement.status === 'terminal'
                  ? (decoded.settlement.winner === index
                    ? 'winning · pays out under the Market’s own basis (one atom per claim atom when categorical)'
                    : 'losing · pays zero')
                  : 'unsettled · the terminal receipt decides what this claim pays';
                return outcome === undefined ? status : `${outcome} (editorial name) · ${status}`;
              })}
            />
            <ol className="outcome-vector">
              {decoded.liability.supplyAtoms.map((amount, index) => {
                const outcomeName = editorial !== null && editorial.outcomes !== null ? editorial.outcomes[index] : undefined;
                return (
                <li key={index} className={decoded.settlement.status === 'terminal' && decoded.settlement.winner === index ? 'winning-outcome' : ''}>
                  <span>claim {index}{outcomeName === undefined ? '' : ` · ${outcomeName}`}</span>
                  <strong>{amount}</strong>
                  {decoded.settlement.status === 'terminal' && <small>{decoded.settlement.winner === index ? 'winning · pays out under the Market\u2019s own basis (one atom per claim atom when categorical)' : 'losing · pays zero'}</small>}
                </li>
                );
              })}
            </ol>
            {editorial !== null && editorial.outcomes !== null
              && <p className="market-editorial-note">The outcome names beside the claim indices are this site&apos;s editorial; the indices, atoms, and every figure above are the chain&apos;s.</p>}
            <h3 className="detail-subhead">Hoard</h3>
            {decoded.hoard.status === 'derived'
              ? <dl className="detail-facts">
                <Fact label="Principal (atoms)" value={decoded.hoard.principalAtoms} />
                <Fact label="Vault" value={decoded.hoard.address} />
                <Fact label="Custody transfer authority" value={decoded.hoard.custodyAuthority} />
                <ContentId label="Custody namespace" value={decoded.hoard.custodyContext} />
                <Fact label="Custody program" value={decoded.hoard.custodyProgramId} />
                <Fact label="Finalized observed slot" value={decoded.hoard.observedSlot} />
              </dl>
              : <p className="market-capability-refusal"><span>Hoard {decoded.hoard.status}</span>{decoded.hoard.reason}</p>}
            <h3 className="detail-subhead">Terminal settlement</h3>
            {decoded.settlement.status === 'terminal'
              ? <dl className="detail-facts">
                <Fact label="State" value={decoded.settlement.label} />
                <Fact label="Winning claim" value={String(decoded.settlement.winner)} />
                <ContentId label="Terminal receipt ID" value={decoded.settlement.receiptId} />
              </dl>
              : <p className="market-hoard-note">No terminal receipt is written, so no claim is winning and no claim can be redeemed. This is the account&apos;s own state, not a missing read.</p>}
          </>}
    </section>}

    {refused === null && <section className="trade-v3-card">
      <header><span>03</span><div><h2>Realm</h2><p>The Market names its Realm by content identity. The Realm account read here is the content-addressed program address of that identity, so the collateral binding is derived, never supplied.</p></div><SectionProvenance provenance={realmProvenance} /></header>
      {decoded === null
        ? <p className="market-empty">No Realm was reacquired, because no Market root has been decoded.</p>
        : <Realm collateral={decoded.collateral} />}
    </section>}

    {refused === null && <section className="trade-v3-card">
      <header><span>04</span><div><h2>Capabilities</h2><p>A capability exists only if this Market&apos;s own authenticated manifest lists it. Each entry opens to its exact identities, activation policy, dependency list, and immutable funding quote — quoted in seven segregated compartments with separate native-lamport and Realm-collateral totals, never merged into one number.</p></div><SectionProvenance provenance={capabilityProvenance} /></header>
      {decoded === null
        ? <p className="market-empty">No capability manifest identity exists to authenticate, because no Market root has been decoded.</p>
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

    <footer className="product-footer">
      <span>Chain-derived fields, atoms, and refusals only</span>
      <span>Hoard principal is never presented as an available balance</span>
    </footer>
  </main>;
}
