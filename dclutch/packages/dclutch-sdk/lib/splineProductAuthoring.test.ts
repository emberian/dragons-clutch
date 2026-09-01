import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { hex, sha256 } from './bytes';
import {
  GRADED_BASIS_RECORD_SCHEMA_ID_V3,
  PORTFOLIO_SCHEMA_ID_V2,
  PRICE_GATE_RECORD_SCHEMA_ID_V1,
  PRODUCT_RECORD_SCHEMA_ID_V2,
  RESULT_DOMAIN_SCHEMA_ID_V2,
} from './generated/coreFound';
import { deriveFinalizedRecordAddressesV1 } from './releaseRegistry';
import { inspectSplineProductAuthoringArtifactsV1, type SplineProductArtifactFilesV1 } from './splineProductAuthoring';

function id(byte: number): Uint8Array { return new Uint8Array(32).fill(byte); }

async function fixture(): Promise<Readonly<{ report: Record<string, unknown>; files: SplineProductArtifactFilesV1 }>> {
  const registryProgram = new PublicKey(id(200)).toBase58();
  const files = Object.freeze({
    product: new Uint8Array(112).fill(1),
    resultDomain: new Uint8Array(256).fill(2),
    portfolio: new Uint8Array(232).fill(3),
    productBasis: new Uint8Array(400).fill(4),
    priceGate: new Uint8Array(320).fill(5),
  });
  const definitions = [
    ['product', 'product.bin', files.product, PRODUCT_RECORD_SCHEMA_ID_V2],
    ['result_domain', 'result-domain.bin', files.resultDomain, RESULT_DOMAIN_SCHEMA_ID_V2],
    ['portfolio', 'portfolio.bin', files.portfolio, PORTFOLIO_SCHEMA_ID_V2],
    ['product_basis', 'product-basis.bin', files.productBasis, GRADED_BASIS_RECORD_SCHEMA_ID_V3],
    ['price_gate', 'price-gate.bin', files.priceGate, PRICE_GATE_RECORD_SCHEMA_ID_V1],
  ] as const;
  const records: Record<string, unknown> = {};
  for (const [name, file, bytes, schema] of definitions) {
    const digest = await sha256(bytes);
    const coordinate = deriveFinalizedRecordAddressesV1(registryProgram, schema, digest);
    records[name] = {
      file,
      bytes: bytes.length,
      schema_id: hex(schema),
      content_sha256: hex(digest),
      raw_account: coordinate.record,
      staging_account: coordinate.staging,
    };
  }
  return Object.freeze({
    files,
    report: {
      schema: 'dclutch/product-spline-authoring-report/v1',
      command: 'product-spline-compile-v1',
      key_free: true,
      signs: false,
      submits: false,
      input_sha256: '11'.repeat(32),
      registry_program: registryProgram,
      product_outcome_count: 3,
      basis_width: 3,
      degree: 2,
      interior_multiplicity: false,
      payout_scale: '7',
      rounding_boundary: 'cumulative-floor-v3',
      semantic_basis_id: '22'.repeat(32),
      records,
      verified_price_gate: { scale: 7, mass: '1', degree: 2, width: 3, atom_count: 1, prices: ['1', '4', '2'] },
      // The seventeenth key. `spline_product.rs`'s `ReportV1` has emitted it
      // since the partition-quality gate landed; this fixture described a
      // compiler that no longer exists, and `exactKeys` was refusing every
      // real report with "missing or unknown fields".
      partition_quality: {
        model: 'triangular-plausible-band-v1',
        anchor: '15000000000',
        volatility_bps: 2_000,
        window_slots: '10000',
        characteristic_displacement: '3000000000',
        plausible_half_width: '6000000000',
        dominant_cell: 1,
        dominant_share_bps: 5_000,
        max_cell_share_bps: 9_000,
        cell_share_bps: [2_500, 5_000, 2_500],
      },
    },
  });
}

/** The fixture's partition-quality section, with one field replaced. */
function withQuality(
  report: Record<string, unknown>,
  overrides: Record<string, unknown>,
): Record<string, unknown> {
  return { ...report, partition_quality: { ...(report.partition_quality as Record<string, unknown>), ...overrides } };
}

describe('spline Product authoring artifact handoff', () => {
  it('verifies all compiler bytes and returns the exact Found39 record coordinates', async () => {
    const value = await fixture();
    const inspected = await inspectSplineProductAuthoringArtifactsV1(value.report, value.files);
    expect(inspected.keyFree).toBe(true);
    expect(inspected.signs).toBe(false);
    expect(inspected.submits).toBe(false);
    expect(inspected.foundRecords).toEqual({
      productRecord: inspected.records.product.rawAccount,
      resultDomainRecord: inspected.records.result_domain.rawAccount,
      portfolioRecord: inspected.records.portfolio.rawAccount,
      linkedBasisRecord: inspected.records.product_basis.rawAccount,
      priceGateRecord: inspected.records.price_gate.rawAccount,
    });
  });

  it('refuses byte substitution, unknown report fields, and forged Registry coordinates', async () => {
    const value = await fixture();
    const substituted = { ...value.files, priceGate: new Uint8Array(value.files.priceGate).fill(9) };
    await expect(inspectSplineProductAuthoringArtifactsV1(value.report, substituted)).rejects.toThrow(/content differs/);
    await expect(inspectSplineProductAuthoringArtifactsV1({ ...value.report, unchecked: true }, value.files)).rejects.toThrow(/missing or unknown fields/);
    const records = value.report.records as Record<string, Record<string, unknown>>;
    const forged = { ...value.report, records: { ...records, price_gate: { ...records.price_gate, raw_account: new PublicKey(id(99)).toBase58() } } };
    await expect(inspectSplineProductAuthoringArtifactsV1(forged, value.files)).rejects.toThrow(/noncanonical Registry coordinates/);
  });
});

/**
 * Partition quality: the number that says whether a market is degenerate
 * before it is founded.
 *
 * Measured 2026-09-01, before this section existed: a report carrying the
 * `partition_quality` key the Rust compiler has been emitting was refused
 * outright by `exactKeys` with "spline compiler report has missing or unknown
 * fields", so `dclutch product inspect` could not read ANY current compiler
 * output. The inspector was fail-closed and therefore honest; it was also
 * broken, which is the shape this lane keeps finding — a document assembled by
 * enumeration describing an older version of the object.
 *
 * These tests check consistency, never the model. The triangular displacement
 * measure has one owner in Rust and is not reimplemented here.
 */
describe('partition quality', () => {
  it('carries every measured field through to the caller', async () => {
    const value = await fixture();
    const inspected = await inspectSplineProductAuthoringArtifactsV1(value.report, value.files);
    expect(inspected.partitionQuality).toEqual({
      model: 'triangular-plausible-band-v1',
      anchor: '15000000000',
      volatilityBps: 2_000,
      windowSlots: '10000',
      characteristicDisplacement: '3000000000',
      plausibleHalfWidth: '6000000000',
      dominantCell: 1,
      dominantShareBps: 5_000,
      maxCellShareBps: 9_000,
      cellShareBps: [2_500, 5_000, 2_500],
      degenerate: false,
    });
  });

  it('refuses a section that is missing, extended, or renamed', async () => {
    const value = await fixture();
    const without: Record<string, unknown> = { ...value.report };
    delete without.partition_quality;
    await expect(inspectSplineProductAuthoringArtifactsV1(without, value.files)).rejects.toThrow(/missing or unknown fields/);
    await expect(inspectSplineProductAuthoringArtifactsV1(withQuality(value.report, { extra: 1 }), value.files))
      .rejects.toThrow(/report\.partition_quality has missing or unknown fields/);
  });

  it('refuses a displacement model it cannot describe rather than rendering it as this one', async () => {
    const value = await fixture();
    await expect(inspectSplineProductAuthoringArtifactsV1(withQuality(value.report, { model: 'uniform-band-v2' }), value.files))
      .rejects.toThrow(/names a displacement model this client cannot describe/);
  });

  it('refuses shares that do not sum to one unit within their flooring slack', async () => {
    const value = await fixture();
    await expect(inspectSplineProductAuthoringArtifactsV1(withQuality(value.report, { cell_share_bps: [2_500, 5_000, 2_501], dominant_share_bps: 5_000 }), value.files))
      .rejects.toThrow(/does not sum to one unit within its flooring slack/);
    await expect(inspectSplineProductAuthoringArtifactsV1(withQuality(value.report, { cell_share_bps: [10, 20, 30], dominant_cell: 2, dominant_share_bps: 30 }), value.files))
      .rejects.toThrow(/does not sum to one unit within its flooring slack/);
    // Three cells may floor away at most three basis points, and exactly three
    // is admitted: the producer's own bound, not one basis point tighter.
    await expect(inspectSplineProductAuthoringArtifactsV1(withQuality(value.report, { cell_share_bps: [2_499, 4_999, 2_499], dominant_cell: 1, dominant_share_bps: 4_999 }), value.files))
      .resolves.toBeDefined();
  });

  it('refuses a dominant cell that is not the cell holding the most mass', async () => {
    const value = await fixture();
    await expect(inspectSplineProductAuthoringArtifactsV1(withQuality(value.report, { dominant_cell: 0, dominant_share_bps: 2_500 }), value.files))
      .rejects.toThrow(/is not the cell holding the most ex-ante mass/);
    await expect(inspectSplineProductAuthoringArtifactsV1(withQuality(value.report, { dominant_share_bps: 4_000 }), value.files))
      .rejects.toThrow(/is not the cell holding the most ex-ante mass/);
  });

  it('refuses a degenerate partition its own compiler must already have refused', async () => {
    const value = await fixture();
    await expect(inspectSplineProductAuthoringArtifactsV1(
      withQuality(value.report, { cell_share_bps: [0, 0, 10_000], dominant_cell: 2, dominant_share_bps: 10_000 }),
      value.files,
    )).rejects.toThrow(/states a degenerate partition \(cell 2 takes 10000 of 9000 permitted basis points\)/);
  });
});
