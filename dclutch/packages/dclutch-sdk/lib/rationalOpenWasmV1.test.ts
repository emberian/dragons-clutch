import { beforeAll, describe, expect, it } from 'vitest';

import {
  RATIONAL_OPEN_NATIVE_PARITY_INPUT_V1,
  RATIONAL_OPEN_NATIVE_PARITY_OUTPUT_V1,
} from './generated/rationalOpenWasmV1';
import {
  loadRationalOpenWasmV1,
  parseRationalOpenWasmPlanV1,
} from './rationalOpenWasmV1';
import { loadRationalOpenWasmV1ForTest } from './rationalOpenWasmV1.testSupport';

describe('Rational-open native/WASM crossing', () => {
  beforeAll(loadRationalOpenWasmV1ForTest);

  it('is byte-exact with the native Rust owner on one generated parity fixture', async () => {
    const wasm = await loadRationalOpenWasmV1();
    const output = wasm.plan_rational_open_v1(RATIONAL_OPEN_NATIVE_PARITY_INPUT_V1);
    expect(output).toBe(RATIONAL_OPEN_NATIVE_PARITY_OUTPUT_V1);
    await expect(parseRationalOpenWasmPlanV1(output)).resolves.toMatchObject({
      action: 'issue-structured', assetCount: 2, logicalClaimsAccounts: 40,
      rawQuantity: 2n, rawReceiptDelta: 2n, rawShardDeltas: [20n, 20n],
    });
  });

  it('independently refuses a substituted family digest', async () => {
    const raw = JSON.parse(RATIONAL_OPEN_NATIVE_PARITY_OUTPUT_V1) as Record<string, unknown>;
    raw.familySha256 = '00'.repeat(32);
    await expect(parseRationalOpenWasmPlanV1(JSON.stringify(raw))).rejects.toThrow(/does not authenticate/);
  });
});
