'use client';

import { useEffect, useState } from 'react';

import { useDeploymentV1 } from '@/lib/deploymentStore';
import {
  enumerateCoreMarketAddressesV1,
  inspectMarketDiscoveryV1,
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
 * the chain's own reason. Collateral atoms are summed ONLY when every
 * derived Hoard shares one mint — atoms of different mints are different
 * physical dimensions and are never added, here or anywhere else.
 */

export type PulseState = Readonly<{ stats: ReadonlyArray<NumberStripStatV1>; provenance: string }>;

const UNREAD: ReadonlyArray<NumberStripStatV1> = Object.freeze([
  Object.freeze({ label: 'Current markets listed', value: null, detail: 'markets on this deployment, whatever stage they are at' }),
  Object.freeze({ label: 'Collateral in listed markets', value: null, detail: 'locked in their vaults, in raw units' }),
  Object.freeze({ label: 'Resolutions in listed markets', value: null, detail: 'markets that have reached their answer' }),
]);

type ProgramScanEnumerationV1 = Extract<MarketEnumerationV1, Readonly<{ mode: 'program-scan' }>>;

function incompatibleDisclosure(enumeration: ProgramScanEnumerationV1): string {
  const count = enumeration.incompatibleMarketAccounts.length;
  return count === 0
    ? 'The same scan found no older markets that this page cannot read.'
    : `The same scan found ${count} older market${count === 1 ? '' : 's'} in a layout this page cannot read, so ${count === 1 ? 'it is' : 'they are'} not counted above.`;
}

/**
 * What the strip shows when the scan succeeded and the join did not.
 *
 * The scan is one request; the join is roughly four per market, and against a
 * throttling public endpoint the second can fail while the first has already
 * answered. Blanking all three counts then throws away a number we hold --
 * and on the front page, where a refusal is the first thing anyone reads.
 *
 * So the count stands on the scan that produced it, and the two that genuinely
 * were not read stay dashes. That is the page's own rule: a dash means we
 * could not read it.
 */
export function partiallyReadPulseV1(
  deploymentLabel: string,
  enumeration: ProgramScanEnumerationV1,
  reason: string,
): PulseState {
  return Object.freeze({
    stats: Object.freeze([
      Object.freeze({
        label: 'Current markets listed',
        value: String(enumeration.addresses.length),
        detail: 'markets on this deployment, whatever stage they are at',
      }),
      Object.freeze({ label: 'Collateral in listed markets', value: null, detail: 'not read this time' }),
      Object.freeze({ label: 'Resolutions in listed markets', value: null, detail: 'not read this time' }),
    ]),
    provenance: `Read live from ${deploymentLabel} at slot ${enumeration.scanSlot}: the deployment holds ${enumeration.addresses.length} market${enumeration.addresses.length === 1 ? '' : 's'}. Reading inside them did not finish — ${reason}`,
  });
}

/** The truthful zero-current state, kept pure so the legacy-account case is pinned by tests. */
export function emptyCurrentMarketPulseV1(deploymentLabel: string, enumeration: ProgramScanEnumerationV1): PulseState {
  return Object.freeze({
    stats: Object.freeze([
      Object.freeze({ label: 'Current markets listed', value: '0', detail: 'no market this reader can read exists here yet' }),
      Object.freeze({ label: 'Collateral in listed markets', value: '0', detail: 'there is no market to hold any' }),
      Object.freeze({ label: 'Resolutions in listed markets', value: '0', detail: 'there is no market to resolve' }),
    ]),
    provenance: `Read live from ${deploymentLabel} at slot ${enumeration.scanSlot}: this deployment holds no market this page can read. ${incompatibleDisclosure(enumeration)}`,
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
        const decoded = discovery.cards.filter((card) => card.status === 'decoded');
        const hoards = decoded.map((card) => card.hoard).filter((hoard) => hoard.status === 'derived');
        const mints = new Set(hoards.map((hoard) => hoard.collateralMint));
        // Zero DERIVED Hoards is not the same fact as zero atoms locked: a
        // vault that refused authentication may still hold principal, so the
        // sum is unread rather than zero.
        const locked = hoards.length === 0
          ? { value: null, detail: 'their vaults could not be read, so no total is claimed' }
          : mints.size > 1
            ? { value: null, detail: `${mints.size} different collateral tokens — their units do not add up` }
            : {
              value: hoards.reduce((total, hoard) => total + BigInt(hoard.principalAtoms), 0n).toString(),
              detail: `raw units across ${hoards.length} vault${hoards.length === 1 ? '' : 's'}, one collateral token`,
            };
        const resolutions = decoded.filter((card) => card.settlement.status === 'terminal').length;
        // A market existing and a market being open are different facts, and
        // the count alone conflates them. Devnet currently holds nine Markets,
        // every one of them still in Founding, on a site that correctly says no
        // market is open — a reader seeing "9" and "none is open" deserves the
        // sentence that reconciles them rather than being left to guess.
        const open = decoded.filter((card) => card.phase === 'Open').length;
        settle({
          stats: Object.freeze([
            Object.freeze({
              label: 'Current markets listed',
              value: String(decoded.length),
              detail: open === 0
                ? 'none of them open for trading yet'
                : `${open} open for trading`,
            }),
            Object.freeze({ label: 'Collateral in listed markets', value: locked.value, detail: locked.detail }),
            Object.freeze({ label: 'Resolutions in listed markets', value: String(resolutions), detail: 'markets that have reached their answer' }),
          ]),
          provenance: `Read live from ${deployment.label} at slot ${discovery.floorSlot}, straight from the deployment's own programs. ${incompatibleDisclosure(enumeration)}`,
        });
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
