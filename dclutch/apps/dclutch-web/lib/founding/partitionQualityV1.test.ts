import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import {
  loadPartitionQualityWasmV1,
  parsePartitionQualityReportV1,
  partitionQualityRequestV1,
  requireInterestingPartitionV1,
  type FoundingBeliefV1,
} from './partitionQualityV1';

const wasmPath = fileURLToPath(new URL('../generated/partitionQualityWasm/partition_quality_bg.wasm', import.meta.url));
const exact = () => new Uint8Array(readFileSync(wasmPath));
const load = () => loadPartitionQualityWasmV1((async () => new Response(exact())) as unknown as typeof fetch);

/** A spot-shaped belief: 150.00 with a 200 bp walk over a 10,000-slot window. */
const SPOT: FoundingBeliefV1 = Object.freeze({
  kind: 'spot-band',
  anchor: 15_000n,
  denominator: 100n,
  volatilityBps: 200,
  windowSlots: 10_000n,
  plausibleHalfWidths: 2,
});

describe('the compiled partition-quality gate', () => {
  it('measures a spot-centred partition under the model its belief selects', async () => {
    const gate = await load();
    // Three cuts around spot: the shape a wizard would place.
    const report = requireInterestingPartitionV1(gate, [14_800n, 15_000n, 15_200n], SPOT, 9_000);
    expect(report.model).toBe('triangular-plausible-band-v1');
    expect(report.cellShareBps).toHaveLength(4);
    expect(report.degenerate).toBe(false);
    // The spot model puts the whole plausible band inside the partition, so
    // nothing lands on a non-cell. The residue is flooring, not mass.
    expect(report.unresolvedShareBps).toBe(0);
    expect(report.characteristicDisplacement).not.toBeNull();
    expect(report.cellShareBps.reduce((sum, share) => sum + share, 0)).toBeGreaterThan(9_990);
  }, 30_000);

  it('refuses the convicted case by the compiler’s own name, not a client word for it', async () => {
    const gate = await load();
    // Cuts three orders of magnitude away from spot: every ordinary cell but
    // one lies outside the plausible band, so one cell takes the whole market.
    expect(() => requireInterestingPartitionV1(gate, [15_000_000n, 15_000_100n], SPOT, 9_000))
      .toThrow(/DegenerateOutcomePartition/);
  }, 30_000);

  it('refuses an author ceiling above the ceiling on ceilings', async () => {
    const gate = await load();
    expect(() => requireInterestingPartitionV1(gate, [14_800n, 15_200n], SPOT, 9_500))
      .toThrow(/CellShareCeilingAboveMaximum/);
    expect(gate.partition_quality_maximum_ceiling_bps_v1()).toBe(9_000);
  }, 30_000);

  it('carries the propositional member too, with its own model and its own unresolved mass', async () => {
    const gate = await load();
    const proposition: FoundingBeliefV1 = {
      kind: 'stated-proposition',
      denominator: 1n,
      // Two ordinary cells at 30% and 20%; the remaining half is the market's
      // own disclosed failure outcome, which is not an ordinary cell.
      cellProbabilityBps: [3_000, 2_000],
    };
    const report = requireInterestingPartitionV1(gate, [1n], proposition, 9_000);
    expect(report.model).toBe('stated-categorical-prior-v1');
    expect(report.cellShareBps).toEqual([3_000, 2_000]);
    expect(report.unresolvedShareBps).toBe(5_000);
    // A proposition has no displacement, and the report says `null` rather
    // than a zero that would read as one that was measured.
    expect(report.characteristicDisplacement).toBeNull();
    expect(report.plausibleHalfWidth).toBeNull();
    expect(report.degenerate).toBe(false);
  }, 30_000);

  it('refuses a proposition that is mostly about its own failure', async () => {
    const gate = await load();
    // 500 bp on the cells means 9,500 on the failure outcome. The gate measures
    // that mass BESIDE the cells, which is exactly the arm a browser could not
    // have inferred from the cell shares alone.
    expect(() => requireInterestingPartitionV1(gate, [1n], {
      kind: 'stated-proposition', denominator: 1n, cellProbabilityBps: [300, 200],
    }, 9_000)).toThrow(/DegenerateOutcomePartition/);
  }, 30_000);

  it('executes only the generated blob identity and refuses one changed byte', async () => {
    const changed = exact();
    changed[changed.length - 1]! ^= 1;
    await expect(loadPartitionQualityWasmV1((async () => new Response(changed)) as unknown as typeof fetch))
      .rejects.toThrow(/do not match the generated Rust artifact identity/);
  }, 30_000);

  it('refuses a report that is not the accepted format rather than reading its numbers', () => {
    expect(() => parsePartitionQualityReportV1(JSON.stringify({ format: 'something-else', cellShareBps: [] })))
      .toThrow(/not the exact accepted format/);
  });

  /**
   * The convicted SOL/USD partition, measured by the authority that convicts it.
   *
   * `rangeProtection.test.ts` carried these cuts because the flagship authored
   * market shipped with a band in the wrong units: a Pyth SOL/USD observation
   * is on the order of 10^10 raw atoms and the band was quoted in hundredths,
   * so every outcome landed in the top cell. The weaker check the wizard used
   * to run caught that one case with a provisional "32 band widths" bound.
   *
   * The real gate catches it, AND catches one the weaker check called fine.
   */
  const SOL_USD_CUTS = [12_000n, 18_000n] as const;
  const solUsdBelief = (anchor: bigint, volatilityBps: number): FoundingBeliefV1 => ({
    kind: 'spot-band', anchor, denominator: 100n, volatilityBps, windowSlots: 10_000n, plausibleHalfWidths: 2,
  });

  it('refuses the shipped SOL/USD band against a real Pyth observation', async () => {
    const gate = await load();
    expect(() => requireInterestingPartitionV1(gate, [...SOL_USD_CUTS], solUsdBelief(15_000_000_000n, 3_000), 9_000))
      .toThrow(/DegenerateOutcomePartition/);
  }, 30_000);

  it('refuses that band with spot dead centre too, which the weaker check called fine', async () => {
    const gate = await load();
    // 200 bp of 15,000 is a 300-tick displacement, so the whole plausible band
    // sits inside one 6,000-tick cell and that cell takes the market. The
    // deleted unit-sanity check measured DISTANCE FROM THE BAND and saw
    // nothing wrong; the gate measures where the MASS lands and refuses.
    expect(() => requireInterestingPartitionV1(gate, [...SOL_USD_CUTS], solUsdBelief(15_000n, 200), 9_000))
      .toThrow(/DegenerateOutcomePartition/);
  }, 30_000);

  it('admits the same band once the belief says the coordinate reaches it', async () => {
    const gate = await load();
    const report = requireInterestingPartitionV1(gate, [...SOL_USD_CUTS], solUsdBelief(15_000n, 3_000), 9_000);
    expect(report.cellShareBps).toEqual([2_222, 5_555, 2_222]);
    expect(report.dominantCell).toBe(1);
    expect(report.degenerate).toBe(false);
    // Same partition, same spot, different belief. Which is the whole point:
    // a partition is not degenerate on its own.
  }, 30_000);

  it('will not build a request above the cut bound the boundary declares', () => {
    const many = Array.from({ length: 1_025 }, (_unused, index) => BigInt(index));
    expect(() => partitionQualityRequestV1(many, SPOT, 9_000)).toThrow(/at most 1024 cuts/);
  });
});
