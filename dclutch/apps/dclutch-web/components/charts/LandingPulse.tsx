'use client';

import { useEffect, useState } from 'react';

import { useDeploymentV1 } from '@/lib/deploymentStore';
import {
  collateralSubtotalsV1,
  curateMarketListingV1,
  enumerateCoreMarketAddressesV1,
  formatAtomsV1,
  inspectMarketDiscoveryV1,
  type MarketDiscoveryV1,
  type MarketEnumerationV1,
} from '@/lib/marketDiscovery';
import { SolanaRpcClient } from '@/lib/rpc';

import NumberStrip, { type NumberStripStatV1 } from './NumberStrip';

/**
 * The landing's live pulse: three protocol counts read finalized off the
 * active deployment, fed into the presentational NumberStrip.
 *
 * This is the one data-loading shell in components/charts/, and it exists
 * because the landing has no workspace to borrow a read from. It performs
 * exactly one bounded enumeration plus one discovery join per deployment
 * change, and every failure surfaces as the strip's provenance sentence with
 * the chain's own reason. Collateral atoms are summed ONLY within one mint —
 * atoms of different mints are different physical dimensions and are never
 * added, here or anywhere else.
 *
 * WHAT IS COUNTED, and why it changed. The headline used to be how many market
 * accounts exist. That is a true number and the wrong one: a devnet the
 * protocol was built on top of accumulates foundings that were started and
 * left, and the count led with them. Every fact on the strip was honest and
 * the arrangement was not, which is a way of being wrong that no decoder can
 * catch. So the headline is now what a reader came to find out — how many
 * markets are actually open — and the rest of the record follows in the
 * provenance sentence, named and counted, never dropped.
 */

export type PulseState = Readonly<{ stats: ReadonlyArray<NumberStripStatV1>; provenance: string }>;

/**
 * The tile says `Markets open`, not `Markets open for trading`.
 *
 * `Open` is the phase the Core account is literally in, and it is the phase in
 * which a market trades — but whether a given market can be traded against
 * today also depends on a capability root this strip does not read. Naming the
 * phase is exact and costs the reader nothing; promising the action would be
 * this page asserting something it did not check. If the tile ever wants the
 * longer name, the read that earns it is the capability root, not a rewrite.
 */
const OPEN_LABEL = 'Markets open';
const COLLATERAL_LABEL = 'Collateral locked up';
const RESOLVED_LABEL = 'Markets resolved';

const UNREAD: ReadonlyArray<NumberStripStatV1> = Object.freeze([
  Object.freeze({ label: OPEN_LABEL, value: null, detail: 'markets that have finished founding' }),
  Object.freeze({ label: COLLATERAL_LABEL, value: null, detail: 'held in their vaults, in raw units' }),
  Object.freeze({ label: RESOLVED_LABEL, value: null, detail: 'markets that have reached their answer' }),
]);

type ProgramScanEnumerationV1 = Extract<MarketEnumerationV1, Readonly<{ mode: 'program-scan' }>>;

function plural(count: number, one: string, many: string): string {
  return count === 1 ? one : many;
}

function incompatibleDisclosure(enumeration: ProgramScanEnumerationV1): string {
  const count = enumeration.incompatibleMarketAccounts.length;
  return count === 0
    ? 'The same scan found no older markets that this page cannot read.'
    : `The same scan found ${count} older market${plural(count, '', 's')} in a layout this page cannot read, so ${plural(count, 'it is', 'they are')} not counted above.`;
}

/**
 * What the strip shows when the scan succeeded and the join did not.
 *
 * The scan is one request; the join is roughly four per market, and against a
 * throttling public endpoint the second can fail while the first has already
 * answered. Blanking all three counts then throws away a number we hold --
 * and on the front page, where a refusal is the first thing anyone reads.
 *
 * The scan alone cannot say how many markets are OPEN -- that fact lives inside
 * each market, which is the read that failed -- so the tile is a dash and the
 * count the scan did produce is stated in the sentence beneath it. That is the
 * page's own rule: a dash means we could not read it, and a number we hold is
 * never thrown away.
 */
export function partiallyReadPulseV1(
  deploymentLabel: string,
  enumeration: ProgramScanEnumerationV1,
  reason: string,
): PulseState {
  const listed = enumeration.addresses.length;
  return Object.freeze({
    stats: Object.freeze([
      Object.freeze({
        label: OPEN_LABEL,
        value: null,
        detail: `${listed} market${plural(listed, '', 's')} ${plural(listed, 'is', 'are')} listed here; whether ${plural(listed, 'it is', 'they are')} open is read inside ${plural(listed, 'it', 'them')}`,
      }),
      Object.freeze({ label: COLLATERAL_LABEL, value: null, detail: 'not read this time' }),
      Object.freeze({ label: RESOLVED_LABEL, value: null, detail: 'not read this time' }),
    ]),
    provenance: `Read live from ${deploymentLabel} at slot ${enumeration.scanSlot}: the deployment holds ${listed} market${plural(listed, '', 's')}. Reading inside them did not finish — ${reason}`,
  });
}

/** The truthful zero-current state, kept pure so the legacy-account case is pinned by tests. */
export function emptyCurrentMarketPulseV1(deploymentLabel: string, enumeration: ProgramScanEnumerationV1): PulseState {
  return Object.freeze({
    stats: Object.freeze([
      Object.freeze({ label: OPEN_LABEL, value: '0', detail: 'no market this reader can read exists here yet' }),
      Object.freeze({ label: COLLATERAL_LABEL, value: '0', detail: 'there is no market to hold any' }),
      Object.freeze({ label: RESOLVED_LABEL, value: '0', detail: 'there is no market to resolve' }),
    ]),
    provenance: `Read live from ${deploymentLabel} at slot ${enumeration.scanSlot}: this deployment holds no market this page can read. ${incompatibleDisclosure(enumeration)}`,
  });
}

/**
 * The collateral tile, which is the one that had to stop saying "—".
 *
 * Two markets on this deployment each hold half a billion atoms, of two
 * different mints. One figure spanning both would be a number in no unit at
 * all, so the tile used to show a dash and explain that the units do not add
 * up. True, and useless: a dash on this page means "we could not read it", and
 * we had read it exactly. What cannot be added ACROSS tokens adds perfectly
 * WITHIN one, so the tile now shows one exact total per token, in that token's
 * own raw units, and the reader can see for themselves that there are two.
 *
 * The mint's decimals byte, when the mint account authenticated, rides along in
 * each row's label as what it is — a display convention, printed beside the
 * quantity and never multiplied into it.
 */
export function collateralTileV1(discovery: MarketDiscoveryV1): NumberStripStatV1 {
  const rows = collateralSubtotalsV1(discovery.cards);
  if (rows.length === 0) {
    return Object.freeze({
      label: COLLATERAL_LABEL,
      value: null,
      detail: 'no vault here could be authenticated, so no total is claimed',
    });
  }
  const vaults = rows.reduce((total, row) => total + row.vaults, 0);
  return Object.freeze({
    label: COLLATERAL_LABEL,
    value: null,
    parts: Object.freeze(rows.map((row) => Object.freeze({
      value: row.principalAtoms,
      label: [
        row.collateralMintShort,
        row.mintDisplayDecimals === null ? null : `${formatAtomsV1(row.principalAtoms, row.mintDisplayDecimals)} at ${row.mintDisplayDecimals} decimals`,
        `${row.vaults} vault${plural(row.vaults, '', 's')}`,
      ].filter((piece) => piece !== null).join(' · '),
    }))),
    detail: rows.length === 1
      ? `one collateral token, in raw units, across ${vaults} vault${plural(vaults, '', 's')}`
      : `${rows.length} different collateral tokens, each totalled in its own raw units — units of different tokens are never added together`,
  });
}

/**
 * The whole strip, once the join landed. Pure, so the arrangement is testable
 * without a chain: what leads, what is counted, and what the sentence beneath
 * has to disclose about everything the count leaves out.
 */
export function readPulseV1(
  deploymentLabel: string,
  enumeration: ProgramScanEnumerationV1,
  discovery: MarketDiscoveryV1,
): PulseState {
  const listing = curateMarketListingV1(discovery.cards);
  const open = listing.open.length;
  // Counted over every decoded card rather than over the settled group, so a
  // terminal receipt is reported wherever the chain wrote one.
  const resolved = discovery.cards.filter((card) => card.status === 'decoded' && card.settlement.status === 'terminal').length;
  const listed = discovery.cards.length;
  const older = enumeration.incompatibleMarketAccounts.length;

  // The count leads with the two markets anyone came here for; the sentence
  // owes the reader every market the count left out, and what happened to it.
  const rest: string[] = [];
  if (listing.founding.length > 0) {
    rest.push(`${listing.founding.length} ${plural(listing.founding.length, 'is', 'are')} still in founding — earlier attempts from the build-out, left standing because devnet history is public`);
  }
  if (listing.settled.length > 0) {
    rest.push(`${listing.settled.length} ${plural(listing.settled.length, 'has', 'have')} passed its answer and ${plural(listing.settled.length, 'is', 'are')} winding down`);
  }
  if (listing.unreadable.length > 0) {
    rest.push(`${listing.unreadable.length} would not decode and ${plural(listing.unreadable.length, 'carries', 'carry')} its refusal instead of a figure`);
  }

  const breakdown = rest.length === 0
    ? `${listed} market${plural(listed, '', 's')} ${plural(listed, 'is', 'are')} listed on this deployment.`
    : `${listed} market${plural(listed, '', 's')} ${plural(listed, 'is', 'are')} listed on this deployment in all: ${open} open, and ${rest.join('; ')}.`;
  const olderSentence = older === 0
    ? ''
    : ` ${older} more ${plural(older, 'was', 'were')} written by an older version of the protocol, in a layout this page cannot read at all.`;

  return Object.freeze({
    stats: Object.freeze([
      Object.freeze({
        label: OPEN_LABEL,
        value: String(open),
        detail: open === 0
          ? 'none yet — every market here is still being founded'
          : `founding is finished on ${plural(open, 'this one', 'these')}; ${plural(open, 'it holds', 'they hold')} live claims and locked collateral`,
      }),
      collateralTileV1(discovery),
      Object.freeze({
        label: RESOLVED_LABEL,
        value: String(resolved),
        detail: resolved > 0
          ? 'markets that have reached their answer'
          : open === 0
            ? 'none yet — no market is open to resolve'
            : 'none yet — a market reaches its answer when its own source reports, and not before',
      }),
    ]),
    provenance: `Read live from ${deploymentLabel} at slot ${discovery.floorSlot}, straight from the deployment's own programs. ${breakdown}${olderSentence}`,
  });
}

export default function LandingPulse() {
  const deployment = useDeploymentV1();
  const [state, setState] = useState<PulseState>({
    stats: UNREAD,
    provenance: 'Reading live from the chain…',
  });

  useEffect(() => {
    let cancelled = false;
    const settle = (next: PulseState) => { if (!cancelled) setState(next); };
    (async () => {
      settle({ stats: UNREAD, provenance: `Reading live from ${deployment.label}…` });
      let scanned: ProgramScanEnumerationV1 | null = null;
      try {
        const client = new SolanaRpcClient(deployment.endpoint);
        const enumeration = await enumerateCoreMarketAddressesV1(client, deployment.programs.core);
        if (enumeration.mode === 'refused') {
          settle({ stats: UNREAD, provenance: `The ${deployment.label} endpoint would not answer the market scan: ${enumeration.reason}` });
          return;
        }
        if (enumeration.addresses.length === 0) {
          settle(emptyCurrentMarketPulseV1(deployment.label, enumeration));
          return;
        }
        scanned = enumeration;
        const discovery = await inspectMarketDiscoveryV1(client, {
          coreProgramId: deployment.programs.core,
          registryProgramId: deployment.programs.registry,
          claimsProgramId: deployment.programs.claims,
          custodyProgramId: deployment.programs.custody,
          addresses: enumeration.addresses,
        });
        settle(readPulseV1(deployment.label, enumeration, discovery));
      } catch (error) {
        const reason = error instanceof Error ? error.message : 'the read failed without a usable reason';
        if (scanned !== null) { settle(partiallyReadPulseV1(deployment.label, scanned, reason)); return; }
        settle({ stats: UNREAD, provenance: `Refused: ${reason}` });
      }
    })();
    return () => { cancelled = true; };
  }, [deployment]);

  return <NumberStrip stats={state.stats} provenance={state.provenance} />;
}
