import { type MarketDiscoveryCardV1 } from '../lib/marketDiscovery';
import { type PortfolioEntryV1, type PortfolioV1 } from '../lib/portfolio';
import { LIVE } from './liveOpenMarket';

/**
 * Synthetic portfolios for the bundle bound, and only for it.
 *
 * The live fixture carries one Market, which is the wrong shape for a question
 * that only exists across two or more. These builders assemble the exact
 * projected types the bundle arithmetic consumes — the same `PortfolioV1` that
 * `inspectPortfolioV1` returns — so a suite can put two Markets with the same
 * terms identity, or two collateral mints, in front of it without inventing a
 * second set of real account bytes. Every field that the arithmetic reads is
 * set deliberately here; the rest is filled with the honest `unread` variants
 * rather than with plausible-looking numbers.
 *
 * The live fixture stays the ground truth for the decode path, and
 * `bundleExposure.test.ts` exercises both.
 */

export const BUNDLE_SLOT_V1 = '5150';
export const BUNDLE_MINT_A_V1 = 'So11111111111111111111111111111111111111112';
export const BUNDLE_MINT_B_V1 = 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v';

export function bundleHexIdV1(byte: number): string {
  return byte.toString(16).padStart(2, '0').repeat(32);
}

export type BundleTermsV1 = Readonly<{ realm: string; record: string; instance: string; policy: string }>;

/** Two terms identities that agree on nothing a netting theorem could use. */
export const BUNDLE_TERMS_ONE_V1: BundleTermsV1 = Object.freeze({
  realm: bundleHexIdV1(1), record: bundleHexIdV1(2), instance: bundleHexIdV1(3), policy: bundleHexIdV1(4),
});
export const BUNDLE_TERMS_TWO_V1: BundleTermsV1 = Object.freeze({
  realm: bundleHexIdV1(1), record: bundleHexIdV1(2), instance: bundleHexIdV1(9), policy: bundleHexIdV1(10),
});

export type BundleEntryOptionsV1 = Readonly<{
  terms?: BundleTermsV1;
  mint?: string;
  settled?: boolean;
  realmUnread?: boolean;
  basis?: string;
  marketRefused?: boolean;
}>;

function card(address: string, options: BundleEntryOptionsV1): MarketDiscoveryCardV1 {
  const terms = options.terms ?? BUNDLE_TERMS_ONE_V1;
  const mint = options.mint ?? BUNDLE_MINT_A_V1;
  return Object.freeze({
    status: 'decoded',
    address,
    provenance: Object.freeze({ kind: 'chain', observedSlot: BUNDLE_SLOT_V1 }),
    observedSlot: BUNDLE_SLOT_V1,
    phase: options.settled === true ? 'Terminal' : 'Open',
    readiness: 'Ready',
    generation: '1',
    outstandingCapabilities: '0',
    principalCapSets: '500000000',
    settlement: options.settled === true
      ? Object.freeze({ status: 'terminal', label: 'terminal receipt accepted', winner: 1, receiptId: bundleHexIdV1(0x77) })
      : Object.freeze({ status: 'open', label: 'no terminal receipt' }),
    identity: Object.freeze({
      schemaMagic: 'DCLTCOR3',
      schemaVersion: 3,
      accountBytes: 368,
      marketId: address,
      realmId: terms.realm,
      productRecordId: terms.record,
      productInstanceId: terms.instance,
      resolutionPolicyId: terms.policy,
      capabilityManifestId: bundleHexIdV1(5),
      selectedReleaseSetId: bundleHexIdV1(6),
      registryProgram: LIVE.programs.registry,
      rentBeneficiary: LIVE.founder,
    }),
    collateral: options.realmUnread === true
      ? Object.freeze({ status: 'unread', realmContentId: terms.realm, reason: 'no Realm record was read at this floor' })
      : Object.freeze({
        status: 'bound',
        observedSlot: BUNDLE_SLOT_V1,
        realmAddress: address,
        realmContentId: terms.realm,
        collateralMint: mint,
        collateralMintShort: mint.slice(0, 4),
        tokenProgram: LIVE.programs.registry,
        adapterReleaseId: bundleHexIdV1(7),
        mintAuthorityPolicy: 'Require absent',
        freezeAuthorityPolicy: 'Require absent',
      }),
    liability: Object.freeze({ status: 'unread', reason: 'the bundle bound reads Positions, not the aggregate' }),
    hoard: Object.freeze({ status: 'unread', reason: 'the bundle bound reads Positions, not the Vault' }),
    capabilities: Object.freeze({ status: 'unread', manifestId: bundleHexIdV1(5), reason: 'the bundle bound reads no capability' }),
    bindings: Object.freeze([]),
    refusal: null,
  }) as MarketDiscoveryCardV1;
}

/** One held Position in one Market, projected exactly as the portfolio read projects it. */
export function bundleEntryV1(
  address: string,
  balances: ReadonlyArray<string>,
  options: BundleEntryOptionsV1 = {},
): PortfolioEntryV1 {
  const market: MarketDiscoveryCardV1 = options.marketRefused === true
    ? Object.freeze({
      status: 'refused',
      address,
      provenance: Object.freeze({ kind: 'refused', reason: 'no account at this address' }),
      observedSlot: BUNDLE_SLOT_V1,
      refusal: 'no account at this address',
    })
    : card(address, options);
  return Object.freeze({
    marketAddress: address,
    positionAddress: `${address}-position`,
    aggregateAddress: `${address}-aggregate`,
    market,
    position: Object.freeze({
      status: 'held',
      address: `${address}-position`,
      provenance: Object.freeze({ kind: 'chain', observedSlot: BUNDLE_SLOT_V1 }),
      observedSlot: BUNDLE_SLOT_V1,
      aggregateAddress: `${address}-aggregate`,
      revision: '1',
      claimCount: balances.length,
      liabilityBasisId: options.basis ?? bundleHexIdV1(0x0b),
      balances: Object.freeze([...balances]),
      claim: Object.freeze({ kind: 'unavailable', note: 'the bundle suites do not exercise the per-Market transition' }),
    }),
  }) as PortfolioEntryV1;
}

export function bundlePortfolioV1(entries: ReadonlyArray<PortfolioEntryV1>): PortfolioV1 {
  return Object.freeze({
    owner: LIVE.founder,
    coreProgramId: LIVE.programs.core,
    claimsProgramId: LIVE.programs.claims,
    registryProgramId: LIVE.programs.registry,
    floorSlot: BUNDLE_SLOT_V1,
    entries: Object.freeze([...entries]),
    reason: 'assembled by the bundle fixtures',
  });
}
