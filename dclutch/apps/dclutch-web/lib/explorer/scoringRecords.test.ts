import { describe, expect, it } from 'vitest';
import * as S from '@dclutch/sdk/generated/scoringRuleV1';
import { decodeAgainstSpec, specForData, specForMagic } from './accountRecords';
import { decodeInstructionData } from './instructions';

function quoteBytes() {
  const bytes = new Uint8Array(S.QUOTE_BYTES);
  bytes.set(new TextEncoder().encode(S.QUOTE_MAGIC));
  const view = new DataView(bytes.buffer);
  view.setUint16(S.QUOTE_VERSION_OFFSET, S.WIRE_VERSION, true);
  view.setUint8(S.QUOTE_OUTCOME_COUNT_OFFSET, S.MIN_OUTCOMES);
  view.setBigUint64(S.QUOTE_SCALE_OFFSET, S.ONE_Q62, true);
  view.setBigUint64(S.QUOTE_PRICES_OFFSET, S.ONE_Q62 / 2n - 1n, true);
  view.setBigUint64(S.QUOTE_PRICES_OFFSET + S.VECTOR_BYTES / S.MAX_OUTCOMES, S.ONE_Q62 / 2n + 1n, true);
  view.setBigUint64(S.QUOTE_FUND_REVISION_OFFSET, 9n, true);
  return bytes;
}

describe('the scoring Dealer in the existing explorer', () => {
  it('keeps exact scaled prices above Number precision and all stored coordinates', () => {
    const bytes = quoteBytes();
    const decoded = decodeAgainstSpec(specForData(bytes)!, bytes);
    expect(decoded.widthCheck.ok).toBe(true);
    const prices = decoded.fields.find((field) => field.offset === S.QUOTE_PRICES_OFFSET)!.value;
    expect(prices).toEqual({ form: 'vector', values: [String(S.ONE_Q62 / 2n - 1n), String(S.ONE_Q62 / 2n + 1n), ...Array(S.MAX_OUTCOMES - S.MIN_OUTCOMES).fill('0')] });
    expect(decoded.fields.find((field) => field.offset === S.QUOTE_FUND_REVISION_OFFSET)!.value).toEqual({ form: 'scalar', text: '9' });
  });

  it('refuses a truncated vector and shows nonzero reserved bytes', () => {
    const bytes = quoteBytes();
    bytes[S.QUOTE_RESERVED_OFFSET] = 1;
    const spec = specForMagic(S.QUOTE_MAGIC)!;
    const decoded = decodeAgainstSpec(spec, bytes);
    expect(decoded.fields.find((field) => field.offset === S.QUOTE_RESERVED_OFFSET)!.value).toMatchObject({ form: 'reserved', zero: false });
    const short = decodeAgainstSpec(spec, bytes.slice(0, S.QUOTE_PRICES_OFFSET + 1));
    expect(short.widthCheck.ok).toBe(false);
    expect(short.fields.find((field) => field.offset === S.QUOTE_PRICES_OFFSET)!.value).toMatchObject({ form: 'refused' });
  });

  it('decodes a fill’s mint as complete sets, independently of its three vectors', () => {
    const bytes = new Uint8Array(S.FILL_REQUEST_BYTES);
    bytes.set(new TextEncoder().encode(S.FILL_REQUEST_MAGIC));
    const view = new DataView(bytes.buffer);
    view.setUint16(S.FILL_REQUEST_VERSION_OFFSET, S.WIRE_VERSION, true);
    view.setBigUint64(S.FILL_REQUEST_MINT_OFFSET, 11n, true);
    view.setBigUint64(S.FILL_REQUEST_PRICES_OFFSET, 23n, true);
    view.setBigUint64(S.FILL_REQUEST_RECEIVE_OFFSET, 37n, true);
    view.setBigUint64(S.FILL_REQUEST_DELIVER_OFFSET, 41n, true);
    const instruction = decodeInstructionData(bytes);
    expect(instruction.body?.spec.magic).toBe(S.FILL_REQUEST_MAGIC);
    const fields = instruction.body!.fields;
    expect(fields.find((field) => field.offset === S.FILL_REQUEST_MINT_OFFSET)!.value).toEqual({ form: 'scalar', text: '11' });
    for (const [offset, first] of [[S.FILL_REQUEST_PRICES_OFFSET, '23'], [S.FILL_REQUEST_RECEIVE_OFFSET, '37'], [S.FILL_REQUEST_DELIVER_OFFSET, '41']] as const) {
      const value = fields.find((field) => field.offset === offset)!.value;
      expect(value.form).toBe('vector');
      if (value.form === 'vector') expect(value.values[0]).toBe(first);
    }
  });
});
