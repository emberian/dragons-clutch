import { describe, expect, it } from 'vitest';

import {
  SOLANA_RETURN_DATA_MAX_BYTES_V1,
  decodeTransactionReturnDataV1,
} from './transactionReturnData';

const PRODUCER = '11111111111111111111111111111111';

function encoded(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

describe('finalized transaction return data', () => {
  it('makes absence explicit', () => {
    expect(decodeTransactionReturnDataV1(undefined)).toBeNull();
    expect(decodeTransactionReturnDataV1(null)).toBeNull();
  });

  it('preserves one exact 280-byte common Hot acknowledgment for its future finalizer', () => {
    const ack = new Uint8Array(280);
    ack.set(new TextEncoder().encode('DCLTHAK3'), 0);
    new DataView(ack.buffer).setUint16(8, 3, true);
    new DataView(ack.buffer).setUint16(10, 1, true);
    ack.fill(0x5a, 16);
    const observation = decodeTransactionReturnDataV1({ programId: PRODUCER, data: [encoded(ack), 'base64'] });
    expect(observation?.programId).toBe(PRODUCER);
    expect(observation?.data).toEqual(ack);
    expect(observation?.data).toHaveLength(280);
  });

  it('refuses wrong object shapes and unknown fields', () => {
    for (const value of [
      [],
      {},
      { programId: PRODUCER },
      { programId: PRODUCER, data: ['', 'base64'], extra: true },
      { programId: PRODUCER, data: '' },
      { programId: PRODUCER, data: [''] },
      { programId: PRODUCER, data: ['', 'base64', 'extra'] },
    ]) expect(() => decodeTransactionReturnDataV1(value)).toThrow(/returnData|base64 tuple/);
  });

  it('refuses a malformed or noncanonical producer', () => {
    for (const programId of ['not-a-pubkey', '1', `${PRODUCER}1`, '0'.repeat(32)]) {
      expect(() => decodeTransactionReturnDataV1({ programId, data: ['', 'base64'] })).toThrow('producer');
    }
  });

  it('refuses another encoding and malformed or noncanonical base64', () => {
    expect(() => decodeTransactionReturnDataV1({ programId: PRODUCER, data: ['', 'base58'] })).toThrow('base64 tuple');
    for (const data of ['*', 'YQ', 'Y===', 'YQ=\n=']) {
      expect(() => decodeTransactionReturnDataV1({ programId: PRODUCER, data: [data, 'base64'] })).toThrow('canonical bounded base64');
    }
  });

  it('refuses bytes beyond Solana return-data capacity', () => {
    const oversized = new Uint8Array(SOLANA_RETURN_DATA_MAX_BYTES_V1 + 1);
    expect(() => decodeTransactionReturnDataV1({ programId: PRODUCER, data: [encoded(oversized), 'base64'] })).toThrow('canonical bounded base64');
  });
});
