'use client';

import PageShell from '@/components/PageShell';
import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';
import { Card, CardContent } from '@/components/ui/card';
import { type ReactNode, useCallback, useEffect, useRef, useState } from 'react';

import { useDeploymentV1 } from '@/lib/deploymentStore';

import { type CapabilityFundingQuoteV1 } from '@/lib/capabilityManifest';
import {
  inspectMarketDetailV1,
  requiredBackingMeaningV1,
  terminalOutcomeMeaningV1,
  type MarketDetailV1,
} from '@/lib/marketDetail';
import { marketEditorialV1, marketNarrativeV1, type MarketNarrativeV1 } from '@/lib/marketRegistry';
import { formatWindowInstantV1, inspectMarketQuestionV1, type MarketQuestionV1 } from '@/lib/marketQuestion';
import {
  inspectMarketResolutionV1,
  marketRedemptionStateV1,
  type MarketRedemptionStateV1,
  type MarketResolutionV1,
} from '@/lib/marketResolution';
import {
  marketActivationOutlookV1,
  provenanceChipV1,
  shortAddressV1,
  type MarketActivationOutlookV1,
  type MarketCapabilityBadgeV1,
  type MarketCapabilityManifestV1,
  type MarketCollateralV1,
  type MarketDiscoveryCardV1,
  type MarketProvenanceV1,
} from '@/lib/marketDiscovery';
import { collateralDenominationV1 } from '@/lib/marketDenomination';
import { ordinarySelectorJoinV1, type OrdinarySelectorJoinV1 } from '@/lib/ordinarySelectorV1';
import { denominationUnitV1, formatQuantityV1, type DenominationV1 } from '@/lib/quantity';
import CellStrip from '@/components/charts/CellStrip';
import MarketIssuanceHistory from '@/components/charts/MarketIssuanceHistory';
import SupplyShareStrip from '@/components/charts/SupplyShareStrip';
import { formatBasisPointsV1, issuedSupplySharesV1, SUPPLY_SHARE_MEANING_V1 } from '@/lib/supplyShares';
import AggregateRetirementStatus from '@/components/AggregateRetirementStatus';
import MarketActivity from '@/components/MarketActivity';
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
  return <article className="capability-entry">
    <header className="capability-entry-header">
      <span className={`capability-badge${badge.recognized ? ' recognized' : ''}`}>{badge.recognized ? badge.label : `Capability entry ${badge.index}`}</span>
      <small>{badge.activation === 'deadline' ? `switches on by slot ${badge.deadline}${deadlinePhrase(badge.deadline, clock, nowMs)}` : 'switches on immediately'}</small>
    </header>
    <dl className="detail-facts">
      <ContentId label="What kind it is" value={badge.kindId} />
      <ContentId label="Which release runs it" value={badge.programSetId} />
      <ContentId label="How it is configured" value={badge.configId} />
      <Fact label="When it switches on" value={badge.activation} />
      <Fact label="Must be switched on by" value={badge.deadline === null ? 'no deadline — it switches on the moment it is asked to' : `slot ${badge.deadline}${deadlinePhrase(badge.deadline, clock, nowMs)}`} />
      <Fact label="Waits for" value={badge.dependencies.length === 0 ? 'nothing' : badge.dependencies.join(', ')} />
    </dl>
    <FundingQuote funding={badge.funding} />
  </article>;
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

type DecodedMarketV1 = Extract<MarketDiscoveryCardV1, Readonly<{ status: 'decoded' }>>;
type DecisionStatV1 = Readonly<{ label: string; value: string; detail: string }>;

/**
 * WHAT INDEX `terminal_winner` IS, and what this site may call it.
 *
 * TWO CELLS ARE NOW NAMEABLE, on two different authorities, and the difference
 * is the whole point of this function.
 *
 * The LAST cell is pinned by the certificate itself:
 * `bindTerminalResolutionCertificateV2` refuses a failure certificate whose
 * selector is not `outcomeCount - 1` and refuses a success certificate whose
 * selector is, so "the source failed to report" needs no derivation at all.
 *
 * An ORDINARY cell is pinned by ARITHMETIC THIS PAGE CAN NOW PERFORM.
 * `ordinarySelectorJoinV1` mirrors `ResultDomainV2::select_ordinary`, the
 * function the Resolution program runs, and hands it the market's own cuts and
 * the certificate's own observation. When the cell it lands on is the cell the
 * chain committed, the index-ordered label beside it is not a guess: the page
 * reproduced the number rather than assuming its ordering.
 *
 * WHAT THAT REPLACED. Until 2026-09-03 this function named every ordinary cell
 * by number, on the reasoning that the certificate carries `10062091764/1`
 * against cuts of `9900, 10300` over `100` and no exponent joins them. That
 * reasoning was right about the scales and wrong about the join: THE CHAIN
 * APPLIES NO EXPONENT EITHER. It compares the two ratios directly, and every
 * certificate producer pins the observation denominator to 1. So the ordering
 * was checkable all along, and cohort-14b's selector 2 is reproduced from its
 * own records by `ordinarySelector.live.test.ts`.
 *
 * WHAT IT DOES NOT REPLACE. Reproducing the chain's arithmetic says what the
 * protocol DID, never that the cell is right about the world. Cohort-14b's cuts
 * were authored in cents and its observation arrived as raw feed atoms, and the
 * factor between those two units is `StatisticSpecV1.source_scale_exponent` —
 * which that market DECLARES as zero, because it was founded into four bytes
 * that were reserved and enforced zero before `4cd2b9cb5`. The caller now reads
 * that record instead of supplying the number, so the join below is the
 * market's own declared arithmetic rather than a reader's guess at it; what it
 * still cannot say is whether the scale the founding declared is the scale the
 * world uses. `docs/design/OBSERVATION_SCALE_AUTHORITY.md` is where cohort-14b's
 * two readings are printed side by side. The caller says that beside the name;
 * this function only reports which authority the name rests on.
 *
 * With no join supplied, or a join that DISAGREES, the old refusal stands: an
 * ordinary cell is named by its index and `joined` is false. A page that prints
 * a wrong name confidently is worse than one that prints a number and admits
 * what it does not know.
 */
export function terminalWinnerNameV1(
  narrative: MarketNarrativeV1,
  winner: number,
  outcomeCount: number,
  join: OrdinarySelectorJoinV1 | null = null,
): Readonly<{ name: string; joined: boolean; basis: 'certificate-kind' | 'derived-selector' | 'unjoined' }> {
  if (outcomeCount > 0 && winner === outcomeCount - 1) {
    return Object.freeze({
      name: narrative.outcomes?.[winner] ?? 'the source-failure outcome',
      joined: true,
      basis: 'certificate-kind' as const,
    });
  }
  if (join !== null && join.agrees && join.derived === winner) {
    const derived = narrative.outcomes?.[winner];
    if (derived !== undefined) return Object.freeze({ name: derived, joined: true, basis: 'derived-selector' as const });
  }
  return Object.freeze({ name: `claim ${winner}`, joined: false, basis: 'unjoined' as const });
}

export function marketDecisionStatsV1(
  decoded: DecodedMarketV1 | null,
  activation: MarketActivationOutlookV1,
  denomination: DenominationV1 | null,
  narrative: MarketNarrativeV1,
  phaseMeaning: string | null,
  derived: MarketQuestionV1 | null,
  nowMs: number | null,
  join: OrdinarySelectorJoinV1 | null = null,
): readonly [DecisionStatV1, DecisionStatV1, DecisionStatV1, DecisionStatV1] {
  if (decoded === null) {
    const unread = (label: string): DecisionStatV1 => Object.freeze({ label, value: '—', detail: 'Not read yet.' });
    return Object.freeze([unread('Status'), unread('Collateral held'), unread('Leading outcome'), unread('Settles')]);
  }

  const winner = decoded.settlement.status === 'terminal'
    ? terminalWinnerNameV1(narrative, decoded.settlement.winner, decoded.liability.status === 'bound' ? decoded.liability.supplyAtoms.length : 0, join).name
    : null;
  const status: DecisionStatV1 = activation.status === 'never'
    ? Object.freeze({ label: 'Status', value: 'Never traded', detail: activation.reason })
    : decoded.settlement.status === 'terminal'
      ? Object.freeze({ label: 'Status', value: `Resolved — ${winner}`, detail: phaseMeaning ?? decoded.settlement.label })
      : decoded.phase === 'Retiring' || decoded.phase === 'Retired'
        ? Object.freeze({ label: 'Status', value: 'Closed', detail: phaseMeaning ?? decoded.phase })
        : Object.freeze({ label: 'Status', value: decoded.phase === 'Open' ? 'Open' : 'Not open', detail: phaseMeaning ?? decoded.phase });

  const collateral: DecisionStatV1 = decoded.hoard.status === 'derived' && denomination !== null
    ? Object.freeze({
      label: 'Collateral held',
      value: `${formatQuantityV1(decoded.hoard.principalAtoms, denomination).display} ${denominationUnitV1(denomination)}`,
      detail: `${decoded.hoard.principalAtoms} atoms · ${shortAddressV1(decoded.hoard.collateralMint, 5)}`,
    })
    : Object.freeze({
      label: 'Collateral held',
      value: '—',
      detail: decoded.hoard.status === 'derived'
        ? 'The collateral display denomination was not available.'
        : decoded.hoard.reason,
    });

  let leading: DecisionStatV1;
  if (decoded.liability.status !== 'bound') {
    leading = Object.freeze({ label: 'Leading outcome', value: '—', detail: decoded.liability.reason });
  } else {
    const shares = issuedSupplySharesV1(decoded.liability.supplyAtoms);
    if (shares === null) {
      leading = Object.freeze({ label: 'Leading outcome', value: 'No claims issued', detail: '0 claims issued.' });
    } else {
      const first = shares.shares[0];
      const leader = shares.shares.reduce((best, candidate) => candidate.basisPoints > best.basisPoints ? candidate : best, first);
      // NO LEADER IS NOT A LEADER AT 25%. A market that has only ever issued
      // complete sets holds exactly the same count on every outcome, so the
      // reduce above returns index 0 by tie-break and the card announced
      // "claim 0 · 25.00%" as though something had chosen it. The shares are
      // equal, which is the whole content: nothing has traded.
      const level = shares.shares.every((candidate) => candidate.basisPoints === first.basisPoints);
      const name = narrative.outcomes?.[leader.index] ?? `claim ${leader.index}`;
      leading = level
        ? Object.freeze({
          label: 'Leading outcome',
          value: 'No leader',
          detail: `Every outcome holds the same ${formatBasisPointsV1(first.basisPoints)} of ${shares.totalAtoms} claims issued — only complete sets, so nothing has picked a side yet.`,
        })
        : Object.freeze({
          label: 'Leading outcome',
          value: `${name} · ${formatBasisPointsV1(leader.basisPoints)}`,
          detail: `${leader.atoms} of ${shares.totalAtoms} claims issued.`,
        });
    }
  }

  /**
   * When it settles, from the market's own window record.
   *
   * This said "No settlement time is published" to every reader of every
   * market, and it was never true: the window is a `WindowSpecV1` record the
   * market's SourceMaterial selects by digest, and it has been on chain since
   * the founding. The three arms below are three different facts and are kept
   * apart -- resolved, a window read, and a window that could not be read with
   * the reason -- because "no time is published" was one string standing in
   * for all three.
   */
  const settles: DecisionStatV1 = decoded.settlement.status === 'terminal'
    ? Object.freeze({ label: 'Settles', value: 'Resolved', detail: narrative.resolution ?? decoded.settlement.label })
    : derived?.window != null
      ? Object.freeze({
        label: 'Settles',
        value: formatWindowInstantV1(derived.window.endUnixSeconds),
        // Past and future are different facts and the page owes the
        // difference: a window that has closed means the observation this
        // market grades against has already happened and only the resolution
        // is outstanding. The comparison is skipped entirely when the clock
        // has not started, rather than guessed from the render time.
        detail: `${nowMs !== null && BigInt(Math.floor(nowMs / 1000)) > derived.window.endUnixSeconds
          ? 'Its window has closed and nothing has resolved it yet'
          : 'Its window closes then'}, read from this market\u2019s own window record. ${narrative.resolution ?? 'How it settles is not on file here.'}`,
      })
      : Object.freeze({
        label: 'Settles',
        value: '—',
        detail: derived === null
          ? narrative.resolution ?? 'The market\u2019s own records have not been read yet.'
          : `The window record did not read: ${derived.windowRefusal ?? 'no reason was given'}.`,
      });

  return Object.freeze([status, collateral, leading, settles]);
}

function MarketDecisionStats({ stats }: Readonly<{ stats: readonly DecisionStatV1[] }>) {
  return <section className="market-decision-stats" aria-label="Market decision facts">
    {stats.map((stat) => <Card className="market-decision-stat" key={stat.label}>
      <CardContent className="p-0">
        <span>{stat.label}</span>
        <strong>{stat.value}</strong>
        <small>{stat.detail}</small>
      </CardContent>
    </Card>)}
  </section>;
}

export default function MarketDetailWorkspace({ address }: Readonly<{ address: string }>) {
  const deployment = useDeploymentV1();
  // Editorial words for this address, if the shipped registry has any. They
  // never gate a read and never stand in for one: an unregistered market
  // renders its address, exactly as before.
  const editorial = marketEditorialV1(address);
  const [state, setState] = useState<State>({ kind: 'loading', message: 'Reading this market…' });
  /**
   * The market's own words for itself, derived from its records.
   *
   * A second read rather than part of `inspectMarketDetailV1` on purpose: the
   * detail read is what the page IS, and a market whose product records cannot
   * be reached must still render everything else. So this one is allowed to
   * fail alone, and when it does the page falls back to the editorial registry
   * exactly as it always did.
   */
  const [derived, setDerived] = useState<MarketQuestionV1 | null>(null);
  /**
   * The certificate this market's own terminal-receipt slot names.
   *
   * A third read, and it fails alone for the same reason the question read
   * does: a market whose certificate account cannot be reached must still
   * render everything else, and `MarketResolutionV1` carries its own refusal
   * with a reason rather than collapsing to null.
   */
  const [resolution, setResolution] = useState<MarketResolutionV1 | null>(null);
  const detail = state.kind === 'ready' ? state.detail : null;
  const card = detail?.card ?? null;
  const decoded = card !== null && card.status === 'decoded' ? card : null;
  /** Resolved once for the page, and handed to every surface that shows a quantity. */
  const denomination = decoded === null ? null : collateralDenominationV1(decoded.hoard, decoded.collateral);
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
    setDerived(null);
    setResolution(null);
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
      let question: MarketQuestionV1 | null = null;
      if (next.card.status === 'decoded' && next.registryProgramId !== null) {
        try {
          question = await inspectMarketQuestionV1(client, {
            registryProgramId: next.registryProgramId,
            address,
            productRecordId: next.card.identity.productRecordId,
            resolutionPolicyId: next.card.identity.resolutionPolicyId,
          });
          setDerived(question);
        } catch {
          // Left null. The page keeps every other read it made, and each
          // surface below says plainly that this half is not on file rather
          // than pretending the market has no question.
          setDerived(null);
        }
      }
      if (next.card.status === 'decoded') {
        // The window is passed when it read, so the observation's standing can
        // be `inside` rather than `unwindowed`. Passing null is not a claim
        // that the market has no window -- the reader has its own word for
        // that -- so this read is never gated on the question read landing.
        setResolution(await inspectMarketResolutionV1(client, {
          card: next.card,
          resolutionProgramId: deployment.programs.resolution,
          floorSlot: next.floorSlot,
          question: question ?? null,
          // The Registry is what turns the certificate's Source-material
          // identity into two account reads, which is how this page learns the
          // scale the market DECLARES rather than assuming the identity. Its
          // absence would be reported by the reader, not silently be a zero.
          registryProgramId: deployment.programs.registry,
        }));
      }
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
  const narrative = marketNarrativeV1(address, decoded?.phase ?? null, editorial, derived);
  /**
   * The answer in words, once, from the decoded settlement and the market's own
   * outcome width. Null until there is an answer to speak about.
   */
  const terminalOutcomeCount = decoded === null || decoded.liability.status !== 'bound' ? 0 : decoded.liability.supplyAtoms.length;
  /**
   * The certificate's committed cell, run back through the market's own cuts.
   *
   * Both halves are already on this page and were never brought together:
   * `derived` carries the partition the operator wrote and `resolution` carries
   * the observation the chain settled on. `ordinarySelectorJoinV1` performs the
   * Resolution program's own comparison over them, so an ordinary cell is named
   * only when this page has REPRODUCED the selector rather than assumed the
   * cut list's order matches it.
   */
  const selectorJoin = derived === null || resolution === null || resolution.status !== 'authenticated' || resolution.scale.status !== 'declared'
    ? null
    : ordinarySelectorJoinV1(
      derived,
      resolution.observation,
      resolution.selector,
      // THE MARKET'S OWN NUMBER, read from its own `StatisticSpecV1` by
      // `inspectMarketDeclaredScaleV1` and passed through untouched. This was
      // the literal `0` with a note saying what would make it wrong -- the
      // first market founded WITH a factor -- and the note was right: a page
      // that supplies a scale it did not read is reproducing an arithmetic
      // nobody performed. The join is now withheld entirely when the record
      // did not read, because there is no number a reader may substitute for
      // the one the founding wrote.
      resolution.scale.sourceScaleExponent,
    );
  const terminalWinner = decoded === null || decoded.settlement.status !== 'terminal'
    ? null
    : terminalWinnerNameV1(narrative, decoded.settlement.winner, terminalOutcomeCount, selectorJoin);
  const terminalMeaning = decoded === null || decoded.settlement.status !== 'terminal'
    ? null
    : terminalOutcomeMeaningV1({
      winner: decoded.settlement.winner,
      outcomeCount: terminalOutcomeCount,
      // ONLY A JOINED NAME, and `terminalWinnerNameV1` says on which authority:
      // the certificate's own kind for the failure cell, a reproduced selector
      // for an ordinary one. An unjoined cell hands this sentence nothing,
      // because the page's most confident paragraph is the last place an
      // unchecked join belongs.
      outcomeName: terminalWinner?.joined === true ? terminalWinner.name : undefined,
    });
  const redemption = decoded === null ? null : marketRedemptionStateV1(decoded);
  const decisionStats = marketDecisionStatsV1(decoded, activation, denomination, narrative, detail?.phaseMeaning ?? null, derived, nowMs, selectorJoin);

  /**
   * THE OPERATOR FOLD, built as a list so the region can COUNT it.
   *
   * Four sections used to sit in the reader's own flow, between the answer and
   * the trade: the market's identity and immutable content ids, the Realm it
   * pays out in, the capability manifest's funding compartments, and the
   * retirement checkpoint's four durable steps. Every one of them is a fact
   * about the OPERATOR's side of this market. None of them changes what a
   * reader here concludes, and together they were most of a 6,159px page.
   *
   * They are not removed and they are not summarized: each is the same element
   * it was, whole, one keypress away inside a native `<details>` — which is the
   * whole reason this is a disclosure and not a tab or a `hidden` div. A
   * `<summary>` is focusable, operable with Enter and Space, and announced with
   * its expanded state by every screen reader, with no ARIA to get wrong.
   *
   * The count below is read off this array rather than typed, because a
   * sentence that says "four sections" while three render is the kind of claim
   * this page exists to not make: a market whose Realm did not bind, or whose
   * capability manifest did not authenticate, contributes a REFUSAL instead
   * (rendered after the fold, never inside a closed drawer — a refusal that has
   * to be opened to be seen is a refusal nobody read).
   */
  const operatorFold: ReadonlyArray<Readonly<{ id: string; node: ReactNode }>> = [
    {
      id: 'identity',
      node: <details className="market-detail-drawer">
        <summary>{"In the protocol's own words · identity and immutable content"}</summary>
        <div className="market-detail-drawer-body">
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
        </div>
      </details>,
    },
    ...(decoded !== null && decoded.collateral.status === 'bound'
      ? [{
        id: 'realm',
        node: <details className="market-detail-drawer">
          <summary>Realm · payout asset</summary>
          <div className="market-detail-drawer-body"><Realm collateral={decoded.collateral} /></div>
        </details>,
      }]
      : []),
    ...(decoded !== null && decoded.capabilities.status === 'authenticated'
      ? [{
        id: 'capabilities',
        node: <details className="market-detail-drawer">
          <summary>Capability manifest · funding and releases</summary>
          <div className="market-detail-drawer-body"><Capabilities capabilities={decoded.capabilities} clock={clock} nowMs={nowMs} /></div>
        </details>,
      }]
      : []),
    ...(decoded === null
      ? []
      : [{
        id: 'retirement',
        node: <AggregateRetirementStatus
          endpoint={deployment.endpoint}
          coreProgramId={deployment.programs.core}
          claimsProgramId={deployment.programs.claims}
          marketAddress={address}
          marketPhase={decoded.phase}
          marketGeneration={decoded.generation}
          minimumContextSlot={state.kind === 'ready' ? state.detail.floorSlot : decoded.observedSlot}
          outcomeCount={terminalOutcomeCount}
        />,
      }]),
  ];

  return <PageShell className="product-shell trade-v3-shell" header={<Nav current="/markets" status={`${deployment.label} · read live`} />}>

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow"><Anchor href="/markets">← all markets</Anchor> · Market</p>
        <h1>{narrative.title}<br /><em>{decoded === null ? (state.kind === 'loading' ? 'reading…' : card !== null && card.status === 'refused' ? 'refused' : 'unread') : decoded.phase}</em></h1>
        {narrative.question !== null && <p className="market-question">{narrative.question}</p>}
        {narrative.story !== null && <p className="market-story">{narrative.story}</p>}
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

    <MarketDecisionStats stats={decisionStats} />

    {/* THE ANSWER, IN WORDS, ON THE PAGE ITSELF. `Resolved — <name>` in the
        stat above and a `won`/`lost` beside each cell say WHICH claim won and
        never what it is; a market that settled because its data source went
        quiet read as an outcome with no reason. This first sat under the phase
        meaning and rendered into a COLLAPSED drawer, which is the same as not
        saying it -- the capture that caught that is why it is here, in the
        page's own flow, above the exact values rather than inside them. */}
    {terminalMeaning !== null && <section className="market-answer-meaning">
      <h2>What this answer means</h2>
      <p><strong>{terminalMeaning.headline}</strong></p>
      <p>{terminalMeaning.forTheWinners}</p>
      <p>{terminalMeaning.forEveryoneElse}</p>
    </section>}


    {/* WHAT IT RESOLVED ON, which is the question a prediction market exists to
        answer and which this page could not answer at all until the certificate
        the Market itself names was followed. Cohort-13 resolved with no
        observation and cohort-14b resolved on a price read inside its own
        window; both used to render as the same four words. */}
    {resolution !== null && resolution.status !== 'not-terminal' && <section className="market-answer-meaning">
      <h2>How it got that answer</h2>
      {resolution.status === 'refused'
        ? <p className="market-refusal">The Market names a resolution certificate{resolution.certificate === null ? '' : ` at ${shortAddressV1(resolution.certificate, 5)}`} and this page could not stand behind it: {resolution.reason}</p>
        : <>
          <p><strong>{resolution.sourceReported
            ? 'A data source reported, and the chain settled on what it said.'
            : 'No data source ever reported. The chain settled on the fallback outcome this market named and paid for before it opened.'}</strong>{' '}
            The certificate is <code>{resolution.kind}</code>, read off {shortAddressV1(resolution.certificate, 5)} and joined to this Market&rsquo;s own terminal authority — same market, same generation, same selector, same receipt.</p>
          {resolution.observation !== null && <dl className="detail-facts">
            {/* THE CERTIFICATE'S OWN SCALE, said in the label. It carries a
                ratio and no exponent, so this is not dollars and must not be
                laid beside the market's cuts as though it were -- see
                `terminalWinnerNameV1`. Rendering it anyway is right: it is the
                number the chain committed on, and a reader who has the
                certificate can check this against it. */}
            <Fact label="Observed value · the certificate’s own scale" value={resolution.observation.decimal ?? `${resolution.observation.numerator} ÷ ${resolution.observation.denominator}`} title={`exact ratio ${resolution.observation.numerator}/${resolution.observation.denominator}, as the certificate carries it`} />
            <Fact label="Reported at" value={formatWindowInstantV1(resolution.observation.atUnixSeconds)} title={`${resolution.observation.atUnixSeconds} seconds`} />
            <Fact
              label="Against its window"
              value={resolution.observation.standing === 'inside' ? 'inside it' : resolution.observation.standing === 'unwindowed' ? 'not compared' : `${resolution.observation.standing} it`}
              title={derived?.window == null ? 'this page did not read the window record' : `${formatWindowInstantV1(derived.window.startUnixSeconds)} to ${formatWindowInstantV1(derived.window.endUnixSeconds)}`}
            />
            <Fact label="Attempts" value={String(resolution.attemptIndex + 1)} />
          </dl>}
          <p>
            The chain committed <strong>claim {resolution.selector}</strong>.{' '}
            {terminalWinner?.basis === 'certificate-kind'
              ? 'That is the source-failure cell, which is the one index the certificate itself pins: a failure certificate may carry no other, and a success certificate may not carry this one.'
              : terminalWinner?.basis === 'derived-selector'
                ? <>That is <strong>{terminalWinner.name}</strong>, and this page CHECKED it rather than counting on the list above being in the right order: running this market&rsquo;s own cuts over the certificate&rsquo;s own observation, with the same comparison the Resolution program performs, lands on the cell the chain committed. What the check settles is which cell the protocol chose. What it does not settle is whether that cell is right about the world &mdash; the comparison uses the decimal shift this market&rsquo;s own <code>StatisticSpecV1</code> declares between its observation&rsquo;s unit and its cuts&rsquo;, read from that record and never assumed, and a founding that declared the wrong shift is reproduced faithfully and is still wrong.</>
                : 'This site names it by NUMBER and not by one of the cells listed above. The cell names on this page are derived from the market\u2019s own cut list in ascending order, and running that list over the certificate\u2019s observation does NOT land on the cell the chain committed \u2014 so one of the two readings is wrong and this page cannot say which. Naming the cell would be a sentence nobody checked.'}
          </p>
          {resolution.providerEvidenceId !== null && <details className="market-detail-drawer">
            <summary>The digests this answer is pinned to</summary>
            <div className="market-detail-drawer-body">
              <ContentId label="Provider evidence the certificate pins" value={resolution.providerEvidenceId} />
              <ContentId label="Source material the certificate pins" value={resolution.sourceMaterialId} />
            </div>
          </details>}
        </>}
      {redemption !== null && redemption.status === 'read' && denomination !== null && <p>
        {redemption.progress === 'none'
          ? `Nothing has been cashed in yet: the vault still holds ${formatQuantityV1(redemption.heldAtoms, denomination).display} ${denominationUnitV1(denomination)} against the ${formatQuantityV1(redemption.owedAtoms, denomination).display} the winning claim is owed.`
          : redemption.progress === 'complete'
            ? `It has been paid out. The vault holds nothing, against ${formatQuantityV1(redemption.owedAtoms, denomination).display} ${denominationUnitV1(denomination)} the winning claim was owed \u2014 ${redemption.redeemedAtoms} atoms have left it.`
            : `Partly cashed in: ${redemption.redeemedAtoms} atoms have left the vault and it still holds ${formatQuantityV1(redemption.heldAtoms, denomination).display} ${denominationUnitV1(denomination)}.`}
        {' '}This is read from the vault against what the Claims aggregate says the winners are owed. The Market records no recipient, so this page names none.
      </p>}
    </section>}

    <section className="trade-v3-card route-card">
      <header>
        <span>00</span>
        <div><h2>Market read</h2><p>{deployment.label}</p></div>
        <div className="direct-actions"><button type="button" onClick={() => void read()} disabled={state.kind === 'loading'}>{state.kind === 'loading' ? 'Reading…' : 'Read it again'}</button></div>
      </header>
      <p className="direct-status" aria-live="polite">{state.message}</p>
      {refused !== null && <RefusedMarketStory refusal={refused.refusal} observedSlot={refused.observedSlot} address={address} />}
      <details className="market-detail-drawer">
        <summary>Connection &amp; provenance</summary>
        <div className="market-detail-drawer-body">
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
        <div className="market-provenance-grid">
          <SectionProvenance provenance={marketProvenance} />
          <SectionProvenance provenance={liabilityProvenance} />
          <SectionProvenance provenance={realmProvenance} />
          <SectionProvenance provenance={capabilityProvenance} />
        </div>
        </div>
      </details>
    </section>

    {decoded !== null && decoded.bindings.filter((check) => !check.ok).map((check) => <p className="market-refusal" key={check.label}><strong>{check.label}</strong> {check.detail}</p>)}

    {refused === null && <section className="trade-v3-card">
      <header><span>01</span><div><h2>Where claims sit</h2><p>{SUPPLY_SHARE_MEANING_V1}</p></div></header>
      {decoded === null
        ? <p className="market-empty">Not read yet.</p>
        : decoded.liability.status !== 'bound'
          ? <p className="market-capability-refusal"><span>{decoded.liability.status === 'unread' ? 'liabilities unread' : 'liabilities refused'}</span>{decoded.liability.reason}</p>
          : <>
            <details className="market-detail-drawer">
              <summary>Exact claims ledger and backing</summary>
              <div className="market-detail-drawer-body">
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
              </div>
            </details>
            {/* FE-CHART mount: the cell strip draws the same aggregate the
                list below itemizes; the list stays as the exact-value twin. */}
            <CellStrip
              supplies={decoded.liability.supplyAtoms}
              winner={decoded.settlement.status === 'terminal' ? decoded.settlement.winner : null}
              requiredBackingAtoms={decoded.liability.requiredBackingAtoms}
              requiredBackingNote={requiredBackingMeaningV1(decoded.liability.requiredBackingBasis)}
              caption="Claims issued per outcome, against what this market must be able to pay."
              notes={decoded.liability.supplyAtoms.map((_, index) => {
                const outcome = narrative.outcomes?.[index];
                const status = decoded.settlement.status === 'terminal'
                  ? (decoded.settlement.winner === index ? 'won' : 'lost · pays nothing')
                  : 'no answer yet';
                return outcome === undefined ? status : `${outcome} · ${status}`;
              })}
            />
            <ol className="outcome-vector">
              {decoded.liability.supplyAtoms.map((amount, index) => {
                const outcomeName = narrative.outcomes?.[index];
                return (
                <li key={index} className={decoded.settlement.status === 'terminal' && decoded.settlement.winner === index ? 'winning-outcome' : ''}>
                  <span>claim {index}{outcomeName === undefined ? '' : ` · ${outcomeName}`}</span>
                  <strong>{amount}</strong>
                  {decoded.settlement.status === 'terminal' && <small>{decoded.settlement.winner === index ? 'won' : 'lost · pays nothing'}</small>}
                </li>
                );
              })}
            </ol>
            {/* FE-CHART mount: the same supply vector as the cell strip and
                the ordered list, re-expressed as shares of the whole. */}
            <SupplyShareStrip
              supplies={decoded.liability.supplyAtoms}
              outcomes={narrative.outcomes}
              caption={SUPPLY_SHARE_MEANING_V1}
              emptyReason="No claims issued yet."
            />
            {/* Drawn only for a market some run actually recorded; every other
                market renders nothing here rather than an empty frame. */}
            <MarketIssuanceHistory address={address} outcomes={narrative.outcomes} />
            <details className="market-detail-drawer">
              <summary>Exact vault and answer</summary>
              <div className="market-detail-drawer-body">
            <h3>The vault</h3>
            {decoded.hoard.status === 'derived'
              ? <dl className="detail-facts">
                {/* Humanized above, and the raw atoms keep their own row right
                    below it -- the exact twin is never more than a glance
                    away, and it is never the tooltip alone. */}
                <Fact label="Collateral held" value={`${formatQuantityV1(decoded.hoard.principalAtoms, denomination!).display} ${denominationUnitV1(denomination!)}`} />
                <Fact label="Collateral held (raw)" value={decoded.hoard.principalAtoms} />
                <Fact label="Vault account" value={decoded.hoard.address} />
                <Fact label="Only this may move it" value={decoded.hoard.custodyAuthority} />
                <ContentId label="Under this custody namespace" value={decoded.hoard.custodyContext} />
                <Fact label="Custody program" value={decoded.hoard.custodyProgramId} />
                <Fact label="Read at finalized slot" value={decoded.hoard.observedSlot} />
              </dl>
              : <p className="market-capability-refusal"><span>Vault {decoded.hoard.status}</span>{decoded.hoard.reason}</p>}
            <h3>The answer</h3>
            {decoded.settlement.status === 'terminal'
              ? <dl className="detail-facts">
                <Fact label="State" value={decoded.settlement.label} />
                <Fact label="Outcome that won" value={String(decoded.settlement.winner)} />
                <ContentId label="Fingerprint of the answer" value={decoded.settlement.receiptId} />
              </dl>
              : <p className="market-hoard-note">No answer recorded yet.</p>}
              </div>
            </details>
          </>}
    </section>}

    {/* The page's only past tense, and a read of its own: the crossings this
        market has taken, where the claims sit now, and what the venue is still
        owed. It renders its own refusal rather than taking the page with it. */}
    {decoded !== null && <MarketActivity
      address={address}
      endpoint={deployment.endpoint}
      programs={{
        core: deployment.programs.core,
        registry: deployment.programs.registry,
        trading: deployment.programs.trading,
        claims: deployment.programs.claims,
      }}
      denomination={denomination}
      outcomes={narrative.outcomes}
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
      denomination={denomination!}
      outcomes={narrative.outcomes}
      clock={clock}
      nowMs={nowMs}
    />}

    <section className="operator-fold" aria-labelledby="operator-fold-heading">
      <header>
        <h2 id="operator-fold-heading">For operators and auditors</h2>
        <p>
          {`${operatorFold.length} section${operatorFold.length === 1 ? '' : 's'}`} this page already read
          and a reader does not need in order to know what happened here. Each is closed, and each is
          whole when you open it.
        </p>
      </header>
      {operatorFold.map((entry) => <div key={entry.id}>{entry.node}</div>)}
      {decoded !== null && decoded.collateral.status !== 'bound' && <p className="market-refusal">{decoded.collateral.reason}</p>}
      {decoded !== null && decoded.capabilities.status !== 'authenticated' && <p className="market-capability-refusal"><span>{decoded.capabilities.status === 'unread' ? 'capabilities unread' : 'capabilities refused'}</span>{decoded.capabilities.reason}</p>}
      {/* RELOCATED from the trade panel's footer, where it sat beside the
          explorer link as though the two were the same kind of offer. The
          explorer is where a reader goes to see what this market is connected
          to; the workbench is where an operator drives a route by hand. */}
      <footer className="flow-footer">
        <Anchor className="secondary-action" href="/trade">Advanced: full route workbench →</Anchor>
      </footer>
    </section>

  </PageShell>;
}
