/**
 * The split of issued claims across a market's outcome cells, derived
 * exactly from the Claims aggregate's own supply vector.
 *
 * This is the honest neighbor of "implied odds": the chain stores no order
 * book, no traded price, and no probability — but it stores exactly how many
 * claims of each outcome exist, and the SHARE of issuance sitting on each
 * outcome is a fact a reader may weigh however they like. Every surface that
 * renders these shares must label them as what they are (where issued claims
 * sit) and never as a forecast, a price, or a probability the market "says".
 *
 * Arithmetic is exact: shares are computed on a 1/10000 grid with the
 * largest-remainder method, so they always sum to exactly 100.00% and a
 * nonzero supply never rounds to a phantom 0 share of a smaller vector.
 */

export type SupplyShareV1 = Readonly<{
  /** The claim/outcome index, in the aggregate's own order. */
  index: number;
  /** Issued supply, raw u64 atoms, decimal string — the chain's figure. */
  atoms: string;
  /** Exact share of total issuance in 1/10000 units (2500 = 25.00%). */
  basisPoints: number;
}>;

export type SupplySharesV1 = Readonly<{
  shares: ReadonlyArray<SupplyShareV1>;
  /** Total issued atoms across all cells, decimal string. */
  totalAtoms: string;
  /** True when every cell's issued supply is exactly equal and nonzero. */
  even: boolean;
}>;

/**
 * Derive the issuance split, or null when nothing has been issued at all —
 * a null is "no split exists", never an invented uniform one.
 */
export function issuedSupplySharesV1(supplyAtoms: ReadonlyArray<string>): SupplySharesV1 | null {
  if (supplyAtoms.length === 0) return null;
  const atoms = supplyAtoms.map((value) => BigInt(value));
  if (atoms.some((value) => value < 0n)) throw new Error('issued supply atoms must be unsigned');
  const total = atoms.reduce((sum, value) => sum + value, 0n);
  if (total === 0n) return null;
  const GRID = 10_000n;
  const floors = atoms.map((value) => (value * GRID) / total);
  const remainders = atoms.map((value, index) => ({ index, remainder: (value * GRID) % total }));
  let missing = GRID - floors.reduce((sum, value) => sum + value, 0n);
  // Largest remainder, ties to the lower index: deterministic and exact.
  remainders.sort((a, b) => (a.remainder === b.remainder ? a.index - b.index : a.remainder > b.remainder ? -1 : 1));
  const bumps = new Set<number>();
  for (const { index } of remainders) {
    if (missing === 0n) break;
    bumps.add(index);
    missing -= 1n;
  }
  const first = atoms[0];
  const even = atoms.every((value) => value === first) && first > 0n;
  return Object.freeze({
    shares: Object.freeze(atoms.map((value, index) => Object.freeze({
      index,
      atoms: value.toString(),
      basisPoints: Number(floors[index]) + (bumps.has(index) ? 1 : 0),
    }))),
    totalAtoms: total.toString(),
    even,
  });
}

/** "25.00%" — exact two-decimal rendering of a basis-point share. */
export function formatBasisPointsV1(basisPoints: number): string {
  if (!Number.isInteger(basisPoints) || basisPoints < 0 || basisPoints > 10_000) throw new Error('basis points must be an integer in 0..10000');
  const whole = Math.floor(basisPoints / 100);
  const cents = basisPoints % 100;
  return `${whole}.${String(cents).padStart(2, '0')}%`;
}

/** The label a share surface carries. */
export const SUPPLY_SHARE_MEANING_V1 = 'Claims issued per outcome';
