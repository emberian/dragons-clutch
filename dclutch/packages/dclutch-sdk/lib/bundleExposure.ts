import { type MarketDiscoveryCardV1 } from './marketDiscovery';
import { type PortfolioV1 } from './portfolio';

/**
 * Payout bounds in exact collateral atoms, grouped by collateral mint.
 *
 * Each position pays a nonnegative weighted average of its claim balances,
 * including the failure branch. Its minimum and maximum balances therefore
 * bound every payout. The endpoints need not be attainable for higher-degree
 * splines, so these are bounds rather than predictions of a maximum payout.
 *
 * Compatible positions with identical Realm, Product record, Product instance
 * and resolution-policy identities can be added claim by claim when they all
 * resolve normally. Independent failure outcomes still require the headline
 * to retain the sum of the separate position bounds. Other grids and related
 * statistics need additional basis records before any further netting.
 *
 * Arithmetic uses bigint min, max, addition and subtraction without rounding.
 */

export type BundleLegV1 = Readonly<{
  marketAddress: string;
  positionAddress: string;
  claimCount: number;
  /** Lower payout bound over every outcome and the failure branch. */
  floorAtoms: string;
  /** Upper payout bound, likewise. Attainable for a degree-0 or degree-1 basis. */
  ceilingAtoms: string;
  /** ceiling - floor: the part of this position the outcome decides. */
  swingAtoms: string;
  /** Realm, product record, product instance and resolution policy, joined. */
  termsKey: string;
  liabilityBasisId: string;
  settled: boolean;
}>;

export type BundleClusterV1 =
  | Readonly<{
    status: 'locked';
    termsKey: string;
    marketAddresses: ReadonlyArray<string>;
    sumOfCeilingsAtoms: string;
    sumOfFloorsAtoms: string;
    jointCeilingAtoms: string;
    jointFloorAtoms: string;
    ceilingReleaseAtoms: string;
    floorReleaseAtoms: string;
    note: string;
  }>
  | Readonly<{
    status: 'refused';
    termsKey: string;
    marketAddresses: ReadonlyArray<string>;
    reason: string;
  }>;

export type BundleV1 = Readonly<{
  collateralMint: string;
  collateralMintShort: string;
  legs: ReadonlyArray<BundleLegV1>;
  /** Sum of the leg floors. Owed to this holder under every admissible joint outcome. */
  floorAtoms: string;
  /** Sum of the leg ceilings: an upper bound on the bundle payout. */
  ceilingAtoms: string;
  swingAtoms: string;
  /** Ceiling if every locked pair also resolves together: ceiling - releases. */
  coResolvedCeilingAtoms: string;
  /** Floor under the same condition: floor + releases. */
  coResolvedFloorAtoms: string;
  /** The conditional ceiling release. Exactly '0' when no terms are shared. */
  releaseAtoms: string;
  clusters: ReadonlyArray<BundleClusterV1>;
  sharedTerms: boolean;
  settledLegs: number;
  /** One plain sentence about what the two headline numbers mean. */
  headline: string;
  /** One plain sentence about whether anything nets, and why. */
  netting: string;
  /** One plain sentence stating the monotonicity settlement cannot break. */
  settlement: string;
}>;

export type BundleExclusionV1 = Readonly<{ marketAddress: string; reason: string }>;

export type BundleExposureV1 = Readonly<{
  owner: string;
  floorSlot: string;
  legCount: number;
  bundles: ReadonlyArray<BundleV1>;
  excluded: ReadonlyArray<BundleExclusionV1>;
  /** What no client can derive from a market's identity alone. */
  boundary: string;
  reason: string;
}>;

/** A canonical unsigned decimal atom count, as the exact integer it denotes. */
function atoms(value: string, field: string): bigint {
  if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new Error(`${field} is not a canonical unsigned decimal atom count`);
  return BigInt(value);
}

function low(values: ReadonlyArray<bigint>): bigint {
  return values.reduce((smallest, value) => (value < smallest ? value : smallest), values[0]);
}

function high(values: ReadonlyArray<bigint>): bigint {
  return values.reduce((largest, value) => (value > largest ? value : largest), values[0]);
}

function total(values: ReadonlyArray<bigint>): bigint {
  return values.reduce((sum, value) => sum + value, 0n);
}

/**
 * The exact band one position's payout lies in, over every outcome its market
 * admits and over its frozen failure branch alike.
 *
 * Both ends follow from the one hypothesis the chain enforces on every payout
 * vector: the weights are nonnegative and sum to the collateral denominator. A
 * weighted average of the balances cannot leave the interval its inputs span,
 * and a refund vector is a payout vector too, so failure cannot leave it either.
 */
export function claimBandV1(balances: ReadonlyArray<string>): Readonly<{ floorAtoms: string; ceilingAtoms: string; swingAtoms: string }> {
  if (balances.length === 0) throw new Error('a claim band needs at least one balance');
  const values = balances.map((balance, index) => atoms(balance, `claim ${index} balance`));
  const floor = low(values);
  const ceiling = high(values);
  return Object.freeze({ floorAtoms: floor.toString(), ceilingAtoms: ceiling.toString(), swingAtoms: (ceiling - floor).toString() });
}

/** Realm, product record, product instance and resolution policy, as one key. */
function termsKeyV1(market: Extract<MarketDiscoveryCardV1, Readonly<{ status: 'decoded' }>>): string {
  const { realmId, productRecordId, productInstanceId, resolutionPolicyId } = market.identity;
  return `${realmId}:${productRecordId}:${productInstanceId}:${resolutionPolicyId}`;
}

type WorkingLeg = Readonly<{ leg: BundleLegV1; balances: ReadonlyArray<bigint> }>;

/**
 * Net one set of legs whose markets carry the identical terms identity.
 *
 * They resolve against the same thing under the same payoff, so their claim
 * vectors add in one space and the joint band is the band of the sum. Both
 * releases are theorems (`max` is subadditive, `min` superadditive); a negative
 * one would mean this code is wrong rather than that a holder got a worse deal,
 * so it is refused instead of rendered.
 */
function clusterV1(termsKey: string, members: ReadonlyArray<WorkingLeg>): BundleClusterV1 {
  const marketAddresses = Object.freeze(members.map((member) => member.leg.marketAddress));
  const [first] = members;
  const width = first.leg.claimCount;
  const basis = first.leg.liabilityBasisId;
  for (const member of members) {
    if (member.leg.claimCount !== width) {
      return Object.freeze({
        status: 'refused',
        termsKey,
        marketAddresses,
        reason: `These positions contain ${width} and ${member.leg.claimCount} claims. Their widths differ, so their payout bounds remain separate.`,
      });
    }
    if (member.leg.liabilityBasisId !== basis) {
      return Object.freeze({
        status: 'refused',
        termsKey,
        marketAddresses,
        reason: 'These positions use different liability bases. Matching claim indices may have different payouts, so their bounds remain separate.',
      });
    }
  }

  const summed = Array.from({ length: width }, (_, index) => total(members.map((member) => member.balances[index])));
  const jointCeiling = high(summed);
  const jointFloor = low(summed);
  const sumOfCeilings = total(members.map((member) => atoms(member.leg.ceilingAtoms, 'leg ceiling')));
  const sumOfFloors = total(members.map((member) => atoms(member.leg.floorAtoms, 'leg floor')));
  const ceilingRelease = sumOfCeilings - jointCeiling;
  const floorRelease = jointFloor - sumOfFloors;
  if (ceilingRelease < 0n || floorRelease < 0n) {
    return Object.freeze({
      status: 'refused',
      termsKey,
      marketAddresses,
      reason: 'The joint payout calculation failed its bound check. The total uses the separate position bounds.',
    });
  }

  return Object.freeze({
    status: 'locked',
    termsKey,
    marketAddresses,
    sumOfCeilingsAtoms: sumOfCeilings.toString(),
    sumOfFloorsAtoms: sumOfFloors.toString(),
    jointCeilingAtoms: jointCeiling.toString(),
    jointFloorAtoms: jointFloor.toString(),
    ceilingReleaseAtoms: ceilingRelease.toString(),
    floorReleaseAtoms: floorRelease.toString(),
    note: `These ${members.length} markets have matching Realm, Product record, Product instance and resolution-policy IDs, with compatible claim balances. If all resolve normally, their combined payout is between ${jointFloor} and ${jointCeiling} atoms. This lowers the upper bound by ${ceilingRelease} and raises the lower bound by ${floorRelease}. A market can enter its failure outcome independently after its deadline, so the headline keeps the separate bounds added together.`,
  });
}

function headlineV1(floor: bigint, ceiling: bigint, legs: number): string {
  const subject = legs === 1 ? 'This position' : `This bundle of ${legs} positions`;
  if (ceiling === floor) {
    return `${subject} pays exactly ${ceiling} atoms regardless of the outcome. Equal claim balances give a fixed payout.`;
  }
  return `${subject} pays at least ${floor} atoms and at most ${ceiling}, including failure refunds. The difference of ${ceiling - floor} atoms depends on the outcomes.`;
}

function nettingV1(legs: number, clusters: ReadonlyArray<BundleClusterV1>, release: bigint): string {
  if (legs < 2) {
    return 'One position: the displayed bounds apply to this holding alone.';
  }
  const locked = clusters.filter((cluster) => cluster.status === 'locked').length;
  if (locked === 0) {
    return `No compatible group with identical resolution terms was found among these ${legs} markets. The total adds each position’s lower and upper payout bounds without a netting adjustment.`;
  }
  return `${locked} group${locked === 1 ? ' has' : 's have'} compatible balances and identical resolution terms. If all markets in those groups resolve normally, the combined upper bound is ${release} atoms lower. Other positions retain their separate bounds.`;
}

function settlementV1(settled: number, legs: number): string {
  if (settled === 0) {
    const nothing = legs === 1
      ? 'This market has not settled yet'
      : `None of these ${legs} markets has settled yet`;
    return `${nothing}. Settlement fixes each market’s payout within its bounds. These holdings are fully collateralized.`;
  }
  return `${settled} of ${legs} market${legs === 1 ? '' : 's'} ${settled === 1 ? 'has' : 'have'} settled. Settlement fixes the payout within the displayed bounds; it cannot expand the possible payout range.`;
}

const BOUNDARY_V1 = 'For markets with different payout grids or related statistics, the total uses separate position bounds. Additional netting requires the payoff-basis records.';

/**
 * The model-free bundle exposure of one portfolio read.
 *
 * Bundles are partitioned by collateral mint and never summed across them: atoms
 * of two different mints are two different units, and adding them would be the
 * one arithmetic error on this page that no reader could catch. A Market whose
 * own state or Realm did not decode is excluded by name rather than folded in
 * under a guessed unit.
 */
export function bundleExposureV1(portfolio: PortfolioV1): BundleExposureV1 {
  const excluded: BundleExclusionV1[] = [];
  const byMint = new Map<string, { short: string; members: WorkingLeg[] }>();

  for (const entry of portfolio.entries) {
    const { market, position } = entry;
    if (position.status !== 'held') continue;
    if (market.status !== 'decoded') {
      excluded.push(Object.freeze({
        marketAddress: entry.marketAddress,
        reason: 'The market data could not be decoded. Its collateral mint and terms are unknown, so this position is excluded.',
      }));
      continue;
    }
    if (market.collateral.status !== 'bound') {
      excluded.push(Object.freeze({
        marketAddress: entry.marketAddress,
        reason: `The Realm is ${market.collateral.status}. Its collateral mint is unknown, so this position is excluded.`,
      }));
      continue;
    }
    const band = claimBandV1(position.balances);
    const leg: BundleLegV1 = Object.freeze({
      marketAddress: entry.marketAddress,
      positionAddress: position.address,
      claimCount: position.claimCount,
      floorAtoms: band.floorAtoms,
      ceilingAtoms: band.ceilingAtoms,
      swingAtoms: band.swingAtoms,
      termsKey: termsKeyV1(market),
      liabilityBasisId: position.liabilityBasisId,
      settled: market.settlement.status === 'terminal',
    });
    const mint = market.collateral.collateralMint;
    let bucket = byMint.get(mint);
    if (bucket === undefined) {
      bucket = { short: market.collateral.collateralMintShort, members: [] };
      byMint.set(mint, bucket);
    }
    bucket.members.push(Object.freeze({ leg, balances: Object.freeze(position.balances.map((balance, index) => atoms(balance, `claim ${index} balance`))) }));
  }

  const bundles: BundleV1[] = [];
  for (const [collateralMint, bucket] of byMint) {
    const members = bucket.members;
    const legs = Object.freeze(members.map((member) => member.leg));
    const floor = total(members.map((member) => atoms(member.leg.floorAtoms, 'leg floor')));
    const ceiling = total(members.map((member) => atoms(member.leg.ceilingAtoms, 'leg ceiling')));

    const grouped = new Map<string, WorkingLeg[]>();
    for (const member of members) {
      const group = grouped.get(member.leg.termsKey) ?? [];
      group.push(member);
      grouped.set(member.leg.termsKey, group);
    }
    const clusters: BundleClusterV1[] = [];
    for (const [termsKey, group] of grouped) if (group.length > 1) clusters.push(clusterV1(termsKey, group));

    let ceilingRelease = 0n;
    let floorRelease = 0n;
    for (const cluster of clusters) {
      if (cluster.status !== 'locked') continue;
      ceilingRelease += atoms(cluster.ceilingReleaseAtoms, 'cluster ceiling release');
      floorRelease += atoms(cluster.floorReleaseAtoms, 'cluster floor release');
    }
    const settledLegs = legs.filter((leg) => leg.settled).length;

    bundles.push(Object.freeze({
      collateralMint,
      collateralMintShort: bucket.short,
      legs,
      floorAtoms: floor.toString(),
      ceilingAtoms: ceiling.toString(),
      swingAtoms: (ceiling - floor).toString(),
      coResolvedCeilingAtoms: (ceiling - ceilingRelease).toString(),
      coResolvedFloorAtoms: (floor + floorRelease).toString(),
      releaseAtoms: ceilingRelease.toString(),
      clusters: Object.freeze(clusters),
      sharedTerms: clusters.length > 0,
      settledLegs,
      headline: headlineV1(floor, ceiling, legs.length),
      netting: nettingV1(legs.length, clusters, ceilingRelease),
      settlement: settlementV1(settledLegs, legs.length),
    }));
  }

  const legCount = bundles.reduce((count, bundle) => count + bundle.legs.length, 0);
  const reason = legCount === 0
    ? 'No held positions have both readable market data and a known collateral mint.'
    : bundles.length === 1
      ? legCount === 1
        ? '1 held position.'
        : `${legCount} held positions using one collateral mint.`
      : `${legCount} held positions across ${bundles.length} collateral mints. Each mint has separate totals because their atoms use different units.`;

  return Object.freeze({
    owner: portfolio.owner,
    floorSlot: portfolio.floorSlot,
    legCount,
    bundles: Object.freeze(bundles),
    excluded: Object.freeze(excluded),
    boundary: BOUNDARY_V1,
    reason,
  });
}
