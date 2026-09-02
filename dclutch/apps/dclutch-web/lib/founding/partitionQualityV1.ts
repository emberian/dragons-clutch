import { hex, sha256 } from '../bytes';
import {
  PARTITION_QUALITY_MAX_CUTS_V1,
  PARTITION_QUALITY_REPORT_FORMAT_V1,
  PARTITION_QUALITY_REQUEST_FORMAT_V1,
  PARTITION_QUALITY_WASM_BYTES_V1,
  PARTITION_QUALITY_WASM_SHA256_V1,
} from '../generated/partitionQualityWasmV1';

/**
 * The compiled partition-quality gate, and the browser's half of the seam.
 *
 * THE DEFECT THIS CLOSES. `apps/dclutch-web` had ZERO occurrences of
 * `max_cell_share_bps`, `founding_band`, or volatility-as-input. A market
 * founded through the create wizard was never measured by the gate that
 * refuses degenerate partitions — the wizard ran a strictly weaker unit-sanity
 * check with a provisional constant of its own, and `rangeProtection.ts` said
 * so out loud, lifting plan included.
 *
 * WHAT THIS FILE DELIBERATELY DOES NOT DO is measure anything. The triangular
 * displacement in a second language is the identical defect this application
 * keeps convicting; the lane that shipped the weaker check refused to fix a
 * mirror by building another one, and was right. `require_interesting_partition_v1`
 * is a pure deterministic gate, so it is COMPILED and called.
 *
 * BOTH MEMBERS OF THE BELIEF FAMILY are carried. A belief and its model are one
 * decision — `FoundingBeliefV1::SpotBand` selects the triangular model,
 * `StatedProposition` the stated categorical prior — and a boundary that could
 * express only the spot-shaped one would quietly make propositional markets
 * unauthorable from the browser, which is the market kind the relayed family
 * exists for.
 *
 * A refusal is never softened. `DegenerateOutcomePartition` reaches the reader
 * as that word, from the compiler that raised it.
 */

/** What the author believes about the outcome, and therefore which model measures it. */
export type FoundingBeliefV1 =
  | Readonly<{
    kind: 'spot-band';
    /** Spot coordinate numerator at founding; must be positive. */
    anchor: bigint;
    /** Shared coordinate denominator; must equal the partition's own. */
    denominator: bigint;
    /** Stated volatility in basis points of the anchor over the reference window. */
    volatilityBps: number;
    /** This market's own window, in slots, from founding to deadline. */
    windowSlots: bigint;
    /** How many characteristic displacements the band reaches each way. */
    plausibleHalfWidths: number;
  }>
  | Readonly<{
    kind: 'stated-proposition';
    denominator: bigint;
    /** Ex-ante probability per ordinary cell, in basis points, in cut order. */
    cellProbabilityBps: ReadonlyArray<number>;
  }>;

/** What the gate says about one partition, measured under its own belief. */
export type PartitionQualityReportV1 = Readonly<{
  /** The model the shares were measured under, named by the compiler. */
  model: string;
  ceilingBps: number;
  /** The ceiling on the author's ceiling, from the compiler's own constant. */
  maximumCeilingBps: number;
  /** `null` under a stated prior: a proposition has no displacement. */
  characteristicDisplacement: bigint | null;
  plausibleHalfWidth: bigint | null;
  dominantCell: number;
  dominantShareBps: number;
  cellShareBps: ReadonlyArray<number>;
  /** Stated ex-ante mass landing on NO ordinary cell. */
  unresolvedShareBps: number;
  degenerate: boolean;
}>;

/** The three functions the compiled gate exposes. */
export type PartitionQualityWasmV1 = Readonly<{
  require_interesting_partition_v1_wasm(requestJson: string): string;
  partition_quality_maximum_ceiling_bps_v1(): number;
  partition_quality_basis_points_per_unit_v1(): bigint;
  partition_quality_maximum_volatility_bps_v1(): number;
}>;

function count(value: unknown, field: string): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) {
    throw new Error(`partition quality ${field} is not a count`);
  }
  return value;
}

function optionalDecimal(value: unknown, field: string): bigint | null {
  if (value === null || value === undefined) return null;
  if (typeof value !== 'string' || !/^-?(0|[1-9][0-9]*)$/.test(value)) {
    throw new Error(`partition quality ${field} is not exact decimal text`);
  }
  return BigInt(value);
}

/** The request the gate accepts, built from a partition and its belief. */
export function partitionQualityRequestV1(
  cuts: ReadonlyArray<bigint>,
  belief: FoundingBeliefV1,
  ceilingBps: number,
): string {
  if (cuts.length > PARTITION_QUALITY_MAX_CUTS_V1) {
    throw new Error(`a partition quality request carries at most ${PARTITION_QUALITY_MAX_CUTS_V1} cuts`);
  }
  const wire = belief.kind === 'spot-band'
    ? {
      kind: 'spot-band' as const,
      anchor: belief.anchor.toString(),
      denominator: belief.denominator.toString(),
      volatilityBps: belief.volatilityBps,
      windowSlots: belief.windowSlots.toString(),
      plausibleHalfWidths: belief.plausibleHalfWidths,
    }
    : {
      kind: 'stated-proposition' as const,
      denominator: belief.denominator.toString(),
      cellProbabilityBps: [...belief.cellProbabilityBps],
    };
  return JSON.stringify({
    format: PARTITION_QUALITY_REQUEST_FORMAT_V1,
    cuts: cuts.map((cut) => cut.toString()),
    ceilingBps,
    belief: wire,
  });
}

/**
 * Hostile-decode the gate's own answer.
 *
 * An `error` field carries the compiler's refusal VERBATIM and is thrown as
 * such: `DegenerateOutcomePartition` and `CellShareCeilingAboveMaximum` are the
 * compiler's words, and translating them here would put a second author on the
 * one sentence a founder most needs to be exact.
 */
export function parsePartitionQualityReportV1(source: string): PartitionQualityReportV1 {
  let parsed: unknown;
  try { parsed = JSON.parse(source); } catch { throw new Error('partition quality report is not JSON'); }
  if (parsed === null || typeof parsed !== 'object') throw new Error('partition quality report is not an object');
  const answer = parsed as Record<string, unknown>;
  if (typeof answer.error === 'string') throw new Error(answer.error);
  if (answer.format !== PARTITION_QUALITY_REPORT_FORMAT_V1) {
    throw new Error('partition quality report is not the exact accepted format');
  }
  const shares = answer.cellShareBps;
  if (!Array.isArray(shares)) throw new Error('partition quality report carries no cell shares');
  if (typeof answer.model !== 'string' || answer.model === '') {
    throw new Error('partition quality report names no model');
  }
  return Object.freeze({
    model: answer.model,
    ceilingBps: count(answer.ceilingBps, 'ceiling'),
    maximumCeilingBps: count(answer.maximumCeilingBps, 'maximum ceiling'),
    characteristicDisplacement: optionalDecimal(answer.characteristicDisplacement, 'characteristic displacement'),
    plausibleHalfWidth: optionalDecimal(answer.plausibleHalfWidth, 'plausible half width'),
    dominantCell: count(answer.dominantCell, 'dominant cell'),
    dominantShareBps: count(answer.dominantShareBps, 'dominant share'),
    cellShareBps: Object.freeze(shares.map((share, index) => count(share, `cell share ${index}`))),
    unresolvedShareBps: count(answer.unresolvedShareBps, 'unresolved share'),
    degenerate: answer.degenerate === true,
  });
}

/** Load the checked Rust gate blob; unverified fetched bytes never execute. */
export async function loadPartitionQualityWasmV1(
  fetcher: typeof fetch = (input, init) => globalThis.fetch(input, init),
): Promise<PartitionQualityWasmV1> {
  const url = new URL('../generated/partitionQualityWasm/partition_quality_bg.wasm', import.meta.url);
  const response = await fetcher(url);
  if (!response.ok) throw new Error(`partition quality WASM fetch failed with HTTP ${response.status}`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.length !== PARTITION_QUALITY_WASM_BYTES_V1
      || hex(await sha256(bytes)) !== PARTITION_QUALITY_WASM_SHA256_V1) {
    throw new Error('partition quality WASM bytes do not match the generated Rust artifact identity');
  }
  const wasmModule = await import('../generated/partitionQualityWasm/partition_quality.js');
  await wasmModule.default({ module_or_path: bytes });
  // A blob can match its digest and still come from a different tree. The unit
  // is the one thing every share is quoted in, so a blob counting in something
  // other than basis points would misreport every number without failing.
  const unit = wasmModule.partition_quality_basis_points_per_unit_v1();
  const ceiling = wasmModule.partition_quality_maximum_ceiling_bps_v1();
  if (unit !== 10_000n || ceiling > Number(unit)) {
    throw new Error(`partition quality gate counts in ${String(unit)} per unit with a ${ceiling} ceiling, which is not the compiler's basis-point scale`);
  }
  return Object.freeze({
    require_interesting_partition_v1_wasm: wasmModule.require_interesting_partition_v1_wasm,
    partition_quality_maximum_ceiling_bps_v1: wasmModule.partition_quality_maximum_ceiling_bps_v1,
    partition_quality_basis_points_per_unit_v1: wasmModule.partition_quality_basis_points_per_unit_v1,
    partition_quality_maximum_volatility_bps_v1: wasmModule.partition_quality_maximum_volatility_bps_v1,
  });
}

/** Measure one partition against its own belief, through the compiled gate. */
export function requireInterestingPartitionV1(
  gate: PartitionQualityWasmV1,
  cuts: ReadonlyArray<bigint>,
  belief: FoundingBeliefV1,
  ceilingBps: number,
): PartitionQualityReportV1 {
  return parsePartitionQualityReportV1(
    gate.require_interesting_partition_v1_wasm(partitionQualityRequestV1(cuts, belief, ceilingBps)),
  );
}
