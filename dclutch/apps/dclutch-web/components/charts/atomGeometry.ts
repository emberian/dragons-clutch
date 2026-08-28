/**
 * Exact-to-layout projection for atom-denominated bar charts.
 *
 * Every quantity in this app is a raw u64 atom count carried as a decimal
 * string, and the charts keep it that way: the exact string reaches the
 * reader through labels, readouts, and the table twin. Only the GEOMETRY is
 * projected to numbers, through bigint ratio arithmetic against the tallest
 * value, so a u64 near its ceiling cannot lose precision on the way to a
 * pixel — the division happens in bigint on a 10^6 grid and only the final
 * six-digit fraction becomes a float.
 *
 * No fetching, no formatting policy, no colors: layout only.
 */

const GRID = 1_000_000n;

export type AtomBarLayoutV1 = Readonly<{
  index: number;
  atoms: string;
  /** 0..1 share of the plot height, exact to one millionth. */
  share: number;
  zero: boolean;
}>;

export type AtomBarPlanV1 = Readonly<{
  bars: ReadonlyArray<AtomBarLayoutV1>;
  /** The exact atoms the full plot height represents. */
  ceilingAtoms: string;
  /** 0..1 height of the ceiling reference when one was named, else null. */
  referenceShare: number | null;
  /** Bar thickness in svg units (capped so marks stay thin at small N). */
  barWidth: number;
  /** The 2px surface gap between adjacent bars. */
  gap: number;
  /** Total plot width in svg units. */
  plotWidth: number;
}>;

function exactAtoms(value: string, field: string): bigint {
  if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new Error(`${field} is not a canonical unsigned decimal atom count`);
  return BigInt(value);
}

/** share = atoms / ceiling on a 10^6 grid; exact 0 stays exactly 0. */
function share(atoms: bigint, ceiling: bigint): number {
  if (ceiling === 0n) return 0;
  return Number((atoms * GRID) / ceiling) / 1_000_000;
}

/**
 * Lay out one ordered row of atom bars.
 *
 * The ceiling is the larger of the tallest bar and the named reference (the
 * required-backing line, the merge floor's complete-set count is NOT a
 * ceiling — pass it through `referenceAtoms` only when it bounds the bars
 * from above). All-zero rows are legal and produce zero-height bars: an
 * issued supply of zero is a fact, not an empty state.
 */
export function planAtomBarsV1(
  atoms: ReadonlyArray<string>,
  options?: Readonly<{ referenceAtoms?: string | null }>,
): AtomBarPlanV1 {
  const values = atoms.map((value, index) => exactAtoms(value, `bar ${index}`));
  const reference = options?.referenceAtoms == null ? null : exactAtoms(options.referenceAtoms, 'reference');
  let ceiling = reference ?? 0n;
  for (const value of values) if (value > ceiling) ceiling = value;
  const count = values.length;
  const barWidth = count <= 12 ? 24 : count <= 30 ? 18 : 14;
  const gap = 2;
  return Object.freeze({
    bars: Object.freeze(values.map((value, index) => Object.freeze({
      index,
      atoms: atoms[index],
      share: share(value, ceiling),
      zero: value === 0n,
    }))),
    ceilingAtoms: ceiling.toString(),
    referenceShare: reference === null ? null : share(reference, ceiling),
    barWidth,
    gap,
    plotWidth: count * (barWidth + gap) - gap,
  });
}

/** SVG path for one thin bar: 4px-rounded data end, square at the baseline. */
export function atomBarPathV1(x: number, top: number, baselineY: number, width: number): string {
  const h = baselineY - top;
  if (h <= 0) return '';
  const r = Math.min(4, h, width / 2);
  return `M ${x} ${baselineY} L ${x} ${top + r} Q ${x} ${top} ${x + r} ${top} L ${x + width - r} ${top} Q ${x + width} ${top} ${x + width} ${top + r} L ${x + width} ${baselineY} Z`;
}

/** The exact larger of two decimal atom counts. */
export function maxAtomsV1(left: string, right: string): string {
  return exactAtoms(left, 'left') >= exactAtoms(right, 'right') ? left : right;
}

/** atoms / ceiling as a 0..1 share on the same bigint grid as the bar plan. */
export function atomShareV1(atoms: string, ceilingAtoms: string): number {
  return share(exactAtoms(atoms, 'atoms'), exactAtoms(ceilingAtoms, 'ceiling'));
}

/**
 * Project one exact rational grid onto 0..1 positions.
 *
 * Used by the payout-shape chart for knot x-positions: the numerators are
 * i128-range decimal strings over one shared denominator, so the projection
 * subtracts the first knot and divides by the span in bigint before any
 * float exists.
 */
export function planRationalPositionsV1(numerators: ReadonlyArray<string>): ReadonlyArray<number> {
  const values = numerators.map((value, index) => {
    if (!/^-?(0|[1-9][0-9]*)$/.test(value)) throw new Error(`knot ${index} is not a canonical decimal integer`);
    return BigInt(value);
  });
  if (values.length === 1) return Object.freeze([0]);
  const first = values[0];
  const span = values[values.length - 1] - first;
  if (span <= 0n) throw new Error('knot numerators must be strictly increasing');
  return Object.freeze(values.map((value) => Number(((value - first) * GRID) / span) / 1_000_000));
}
