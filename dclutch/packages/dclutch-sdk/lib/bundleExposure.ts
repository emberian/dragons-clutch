import { type MarketDiscoveryCardV1 } from './marketDiscovery';
import { type PortfolioV1 } from './portfolio';

/**
 * What a bundle of positions can pay, across markets, without a model.
 *
 * A single position's payout is a weighted average of the claim balances it
 * holds: the market's basis hands out weights that are never negative and
 * always sum to one collateral unit, so whatever the market resolves to — and
 * whatever its frozen failure policy refunds if it never resolves at all — the
 * position pays somewhere between its smallest balance and its largest. That is
 * the whole per-market computation. It needs no oracle, no price, no volatility
 * and no correlation; it needs the balances this browser already read.
 *
 * Across markets the question is which combinations of outcomes are possible at
 * once. Two markets that settle against different things exclude nothing: any
 * result of one is compatible with any result of the other, so the bundle's
 * ceiling is exactly the sum of the two ceilings. Not a cautious sum — the
 * exact one, attained at the pair of outcomes that maxes both. Every venue that
 * shows a smaller number there is asserting that the two markets move together.
 * That assertion is a correlation model, and this protocol does not hold one.
 *
 * Netting exists only where the terms themselves overlap. The case this surface
 * implements is the one it can check from the bytes it already has: two markets
 * whose Realm, product record, product instance and resolution policy are all
 * the same identity resolve against the same thing under the same payoff, so
 * their outcomes are locked together. Held jointly those positions add
 * coordinate by coordinate, and `max` is subadditive while `min` is
 * superadditive — the band narrows from both ends, exactly, in integers.
 *
 * That release is stated as CONDITIONAL and never folded into the headline,
 * because the two markets can still come apart: `CommitDeadlineFailure` lets
 * any wallet walk either market to its own failure outcome on its own deadline,
 * one without the other, and the refund a market pays there is written in a
 * record this page does not read. One-sided failure puts the pair off the
 * locked diagonal and back on the sum. So the sum is what the headline says.
 *
 * Everything is exact. There is no division anywhere in this module: the
 * arithmetic is `min`, `max`, addition and subtraction over bigint atoms, so
 * there is no rounding to have a direction and no u64 that can lose its low
 * bits on the way to the screen. The bound reported as "at most" is never
 * rounded down and the bound reported as "at least" is never rounded up.
 *
 * What is NOT computed here, stated as a refusal rather than approximated:
 * markets that share a feed and a window but carry different payoff grids net
 * against each other, and statistics that constrain one another (a window's low
 * can never exceed its close) net further still. Both need the basis records —
 * the knots, the degree, the statistic — and this page reads a market's
 * identity, not its basis. It will not guess them. It also cannot tell a
 * degree-0 or degree-1 basis, whose largest balance is reached exactly, from a
 * degree-2 or degree-3 one, whose interior weights peak below one and so cannot
 * quite reach it. That caveat is live rather than theoretical: this tree admits
 * the categorical degree-0 basis and spline degrees 1 through 3
 * (`SPLINE_MIN_DEGREE_V2` = 1, `SPLINE_MAX_DEGREE_V2` = 3). The ceiling is a
 * true upper bound under all four, exact under two of them, and never
 * understated under any — which is the direction that matters, since this page
 * is read as a promise about the most a holder can be owed.
 */

export type BundleLegV1 = Readonly<{
  marketAddress: string;
  positionAddress: string;
  claimCount: number;
  /** The least this position can pay, over every outcome and the failure branch. */
  floorAtoms: string;
  /** The most it can pay, likewise. Exact for a degree-0 or degree-1 basis. */
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
  /** Sum of the leg ceilings. The exact bundle maximum when no terms are shared. */
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
        reason: `these Markets declare the same terms but their Positions are ${width} and ${member.leg.claimCount} claims wide, so their balances do not add in one space and no netting is claimed`,
      });
    }
    if (member.leg.liabilityBasisId !== basis) {
      return Object.freeze({
        status: 'refused',
        termsKey,
        marketAddresses,
        reason: 'these Markets declare the same terms but their Positions name different liability bases, so the same claim index need not mean the same payout and no netting is claimed',
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
      reason: 'the joint band computed wider than the separate bands, which cannot happen; no netting is claimed from a computation that disagrees with itself',
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
    note: `${members.length} of these Markets settle against the same thing and pay by the same rule, so they cannot land on different answers. Add the balances claim by claim and the pair can pay at most ${jointCeiling} atoms and at least ${jointFloor} — a narrower band than holding them apart by ${ceilingRelease} atoms at the top and ${floorRelease} at the bottom. This holds while both Markets resolve. Either can instead be walked to its own failure outcome on its own deadline by any wallet, which puts them back on the sum, so the figures above the fold stay the sum.`,
  });
}

function headlineV1(floor: bigint, ceiling: bigint, legs: number): string {
  const subject = legs === 1 ? 'This Position' : `Across ${legs} Positions this bundle`;
  if (ceiling === floor) {
    return `${subject} pays exactly ${ceiling} atoms whatever happens. Every claim is held in equal measure, so no outcome moves the total: this is collateral parked rather than a stance on anything.`;
  }
  return `${subject} pays at least ${floor} atoms and at most ${ceiling}, whatever every Market resolves to and whatever any of them refunds if it never resolves at all. ${ceiling - floor} atoms of that are what the outcomes decide; the rest is yours either way.`;
}

function nettingV1(legs: number, clusters: ReadonlyArray<BundleClusterV1>, release: bigint): string {
  if (legs < 2) {
    return 'Netting is a question about two positions or more. With one, the band above is the whole answer.';
  }
  const locked = clusters.filter((cluster) => cluster.status === 'locked').length;
  if (locked === 0) {
    return `These ${legs} Markets settle against different things, so nothing about one rules out anything about another — including all of them going your way at once. The most they can pay together is exactly the sum of what each can pay alone. That sum is the true maximum, not a cautious one. Somewhere else you might be shown a smaller number here, and that number assumes your Markets move together. dClutch holds no opinion about whether they do and will not put one into your arithmetic.`;
  }
  return `${locked} group${locked === 1 ? ' of these Markets settles' : 's of these Markets settle'} against the same thing, and inside a group the outcomes are locked to each other: while every Market in it resolves, ${release} atoms of the sum above can never be paid at once. Everything else here settles against something different, and between those Markets nothing nets without a model — the sum is exactly the answer.`;
}

function settlementV1(settled: number, legs: number): string {
  if (settled === 0) {
    const nothing = legs === 1
      ? 'This Market has not settled yet. When it does'
      : `None of these ${legs} Markets has settled yet. When one does`;
    return `${nothing}, the band above can only narrow: settling takes outcomes out of the set these bounds are taken over, so a settlement can never widen what is left. Nothing here can ask you for more afterwards, because nothing here was ever borrowed.`;
  }
  return `${settled} of ${legs} Market${legs === 1 ? '' : 's'} ${settled === 1 ? 'has' : 'have'} settled. Neither bound above moved the wrong way when that happened, and neither can: settling takes outcomes out of the set these bounds are taken over, so the band only ever narrows. That is the shape of the arithmetic rather than a policy someone chose to honour.`;
}

const BOUNDARY_V1 = 'What this page does not compute, plainly: two Markets can also net when they settle against the same feed over the same window but pay on different grids, and they net further when their statistics constrain one another — a window\'s low can never come out above its close. Both need the payoff basis records themselves, the knots and the degree, and this surface reads a Market\'s identity rather than its basis. It states no number it cannot derive from bytes it read.';

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
        reason: 'the Market did not decode at this finalized floor, so neither its collateral unit nor its terms are known and its Position cannot be added to any bundle',
      }));
      continue;
    }
    if (market.collateral.status !== 'bound') {
      excluded.push(Object.freeze({
        marketAddress: entry.marketAddress,
        reason: `the Market's Realm is ${market.collateral.status}, so the collateral mint these atoms are denominated in is unknown; they are not summed with anything`,
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
    ? 'No held Position in this read carries a decoded Market and a known collateral mint, so there is no bundle to bound. That is the chain state, not an empty state.'
    : bundles.length === 1
      ? legCount === 1
        ? '1 held Position, bounded on its own.'
        : `${legCount} held Positions, all denominated in one collateral mint, bounded together.`
      : `${legCount} held Positions across ${bundles.length} collateral mints. Each mint is bounded on its own; atoms of different mints are different units and are never added.`;

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
