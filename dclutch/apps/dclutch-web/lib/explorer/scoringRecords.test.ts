import { describe, expect, it } from 'vitest';
import { PublicKey } from '@solana/web3.js';
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

function terminalParentBytes(kind: 'redeem' | 'close') {
  const redeem = kind === 'redeem';
  const width = redeem ? S.REDEEM_REQUEST_BYTES : S.CLOSE_REQUEST_BYTES;
  const bytes = new Uint8Array(width + 8); // The child request follows the Dealer-owned prefix.
  const magic = redeem ? S.REDEEM_REQUEST_MAGIC : S.CLOSE_REQUEST_MAGIC;
  const versionOffset = redeem ? S.REDEEM_REQUEST_VERSION_OFFSET : S.CLOSE_REQUEST_VERSION_OFFSET;
  const marketOffset = redeem ? S.REDEEM_REQUEST_MARKET_OFFSET : S.CLOSE_REQUEST_MARKET_OFFSET;
  const dealerOffset = redeem ? S.REDEEM_REQUEST_DEALER_ID_OFFSET : S.CLOSE_REQUEST_DEALER_ID_OFFSET;
  const revisionOffset = redeem
    ? S.REDEEM_REQUEST_EXPECTED_FUND_REVISION_OFFSET
    : S.CLOSE_REQUEST_EXPECTED_FUND_REVISION_OFFSET;
  bytes.set(new TextEncoder().encode(magic));
  const view = new DataView(bytes.buffer);
  view.setUint16(versionOffset, S.WIRE_VERSION, true);
  bytes.fill(redeem ? 0x51 : 0x61, marketOffset, marketOffset + 32);
  bytes.fill(redeem ? 0x52 : 0x62, dealerOffset, dealerOffset + 32);
  view.setBigUint64(revisionOffset, redeem ? 17n : 23n, true);
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

  it.each([
    {
      kind: 'redeem' as const,
      magic: S.REDEEM_REQUEST_MAGIC,
      routeId: 'trading/scoring_dealer_v1::redeem::process_dealer_redeem_v1',
      revisionOffset: S.REDEEM_REQUEST_EXPECTED_FUND_REVISION_OFFSET,
      revision: '17',
    },
    {
      kind: 'close' as const,
      magic: S.CLOSE_REQUEST_MAGIC,
      routeId: 'trading/scoring_dealer_v1::close::process_dealer_close_v1',
      revisionOffset: S.CLOSE_REQUEST_EXPECTED_FUND_REVISION_OFFSET,
      revision: '23',
    },
  ])('renders the generated Dealer $kind prefix and its current census route', ({ kind, magic, routeId, revisionOffset, revision }) => {
    const instruction = decodeInstructionData(terminalParentBytes(kind));
    const marketOffset = kind === 'redeem' ? S.REDEEM_REQUEST_MARKET_OFFSET : S.CLOSE_REQUEST_MARKET_OFFSET;
    const dealerOffset = kind === 'redeem' ? S.REDEEM_REQUEST_DEALER_ID_OFFSET : S.CLOSE_REQUEST_DEALER_ID_OFFSET;
    const marketByte = kind === 'redeem' ? 0x51 : 0x61;
    const dealerByte = kind === 'redeem' ? 0x52 : 0x62;
    expect(instruction.magic).toBe(magic);
    expect(instruction.routes).toEqual([
      expect.objectContaining({ routeId, program: 'trading', handler: routeId.slice('trading/'.length) }),
    ]);
    expect(instruction.body?.spec.magic).toBe(magic);
    expect(instruction.body?.widthCheck.ok).toBe(true);
    expect(instruction.body?.fields.find((field) => field.offset === revisionOffset)?.value).toEqual({
      form: 'scalar', text: revision,
    });
    expect(instruction.body?.fields.find((field) => field.offset === marketOffset)?.value).toEqual({
      form: 'address', base58: new PublicKey(new Uint8Array(32).fill(marketByte)).toBase58(),
    });
    expect(instruction.body?.fields.find((field) => field.offset === dealerOffset)?.value).toEqual({
      form: 'identity', hex: dealerByte.toString(16).padStart(2, '0').repeat(32),
    });
  });
});
