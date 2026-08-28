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
  Object.freeze({ label: 'Current markets listed', value: null, detail: 'compatible finalized Core Market accounts' }),
  Object.freeze({ label: 'Collateral in listed markets', value: null, detail: 'Hoard principal, raw atoms' }),
  Object.freeze({ label: 'Resolutions in listed markets', value: null, detail: 'terminal receipts written' }),
]);

type ProgramScanEnumerationV1 = Extract<MarketEnumerationV1, Readonly<{ mode: 'program-scan' }>>;

function incompatibleDisclosure(enumeration: ProgramScanEnumerationV1): string {
  const count = enumeration.incompatibleMarketAccounts.length;
  return count === 0
    ? 'The Core program owns no historical incompatible Market account at this floor.'
    : `The same scan found ${count} historical DCLTCOR2 Market account${count === 1 ? '' : 's'} in the old 352-byte layout. ${count === 1 ? 'It is' : 'They are'} incompatible with this reader and not listed as current.`;
}

/** The truthful zero-current state, kept pure so the legacy-account case is pinned by tests. */
export function emptyCurrentMarketPulseV1(deploymentLabel: string, enumeration: ProgramScanEnumerationV1): PulseState {
  return Object.freeze({
    stats: Object.freeze([
      Object.freeze({ label: 'Current markets listed', value: '0', detail: 'compatible finalized Core Market accounts' }),
      Object.freeze({ label: 'Collateral in listed markets', value: '0', detail: 'no current compatible Market is listed' }),
      Object.freeze({ label: 'Resolutions in listed markets', value: '0', detail: 'no current compatible Market is listed' }),
    ]),
    provenance: `Read finalized off ${deploymentLabel} at slot ${enumeration.scanSlot}: zero current compatible Markets are listed. ${incompatibleDisclosure(enumeration)}`,
  });
}

export default function LandingPulse() {
  const deployment = useDeploymentV1();
  const [state, setState] = useState<PulseState>({
    stats: UNREAD,
    provenance: 'Reading finalized state from the active deployment…',
  });

  useEffect(() => {
    let cancelled = false;
    const settle = (next: PulseState) => { if (!cancelled) setState(next); };
    (async () => {
      settle({ stats: UNREAD, provenance: `Reading finalized state from ${deployment.label}…` });
      try {
        const client = new SolanaRpcClient(deployment.endpoint);
        const enumeration = await enumerateCoreMarketAddressesV1(client, deployment.programs.core);
        if (enumeration.mode === 'refused') {
          settle({ stats: UNREAD, provenance: `The ${deployment.label} endpoint refused the bounded Market scan: ${enumeration.reason}` });
          return;
        }
        if (enumeration.addresses.length === 0) {
          settle(emptyCurrentMarketPulseV1(deployment.label, enumeration));
          return;
        }
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
          ? { value: null, detail: 'no Hoard was derivable at this floor, so no sum is asserted' }
          : mints.size > 1
            ? { value: null, detail: `${mints.size} different collateral mints — their atoms never add` }
            : {
              value: hoards.reduce((total, hoard) => total + BigInt(hoard.principalAtoms), 0n).toString(),
              detail: `raw atoms across ${hoards.length} Hoard${hoards.length === 1 ? '' : 's'}, one mint`,
            };
        const resolutions = decoded.filter((card) => card.settlement.status === 'terminal').length;
        settle({
          stats: Object.freeze([
            Object.freeze({ label: 'Current markets listed', value: String(decoded.length), detail: 'compatible finalized Core Market accounts' }),
            Object.freeze({ label: 'Collateral in listed markets', value: locked.value, detail: locked.detail }),
            Object.freeze({ label: 'Resolutions in listed markets', value: String(resolutions), detail: 'terminal receipts written' }),
          ]),
          provenance: `Read finalized off ${deployment.label} at floor ${discovery.floorSlot}, from the deployment's own program addresses. ${incompatibleDisclosure(enumeration)}`,
        });
      } catch (error) {
        settle({ stats: UNREAD, provenance: `Refused: ${error instanceof Error ? error.message : 'the read failed without a usable reason'}` });
      }
    })();
    return () => { cancelled = true; };
  }, [deployment]);

  return <NumberStrip stats={state.stats} provenance={state.provenance} />;
}
