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
    },
  });
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
