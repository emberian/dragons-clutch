import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';
import {
  parseStructuredLifecycleIntentV1, structuredLifecycleIntentV1,
  StructuredLifecycleSelectionV1,
} from './structuredLifecycleModelV1';

const market = new PublicKey(new Uint8Array(32).fill(1)).toBase58();
const payer = new PublicKey(new Uint8Array(32).fill(2)).toBase58();
const position = new PublicKey(new Uint8Array(32).fill(3)).toBase58();

describe('Structured lifecycle human intent', () => {
  it('keeps receipt preparation separate from coordinate selection', () => {
    const receipt = structuredLifecycleIntentV1({ market, payer, action: 'activate-receipt', coordinate: '' });
    expect(receipt.coordinate).toBeNull();
    expect(receipt.expectedPosition).toBeNull();
    expect(() => structuredLifecycleIntentV1({ market, payer, action: 'activate-receipt', coordinate: '0' }))
      .toThrow('Receipt actions do not select a coordinate');
    expect(() => structuredLifecycleIntentV1({ market, payer, action: 'retire-receipt', coordinate: '', expectedPosition: position }))
      .toThrow('A Position pin applies only to coordinate actions');
  });

  it('accepts coordinate zero and preserves a Position pin without authoring authority', () => {
    const intent = structuredLifecycleIntentV1({ market, payer, action: 'activate-coordinate', coordinate: '0', expectedPosition: position });
    expect(intent.coordinate).toBe(0);
    expect(intent.expectedPosition).toBe(position);
    expect(parseStructuredLifecycleIntentV1(JSON.parse(JSON.stringify(intent)))).toEqual(intent);
    for (const coordinate of ['01', '-1', '1.5', '4294967296', '1e2']) {
      expect(() => structuredLifecycleIntentV1({ market, payer, action: 'retire-coordinate', coordinate }))
        .toThrow(coordinate === '4294967296' ? 'Representation coordinate exceeds u32' : 'Choose one whole representation coordinate');
    }
  });

  it('refuses a saved intent with extra authority or unsupported action', () => {
    const intent = structuredLifecycleIntentV1({ market, payer, action: 'retire-coordinate', coordinate: '3' });
    expect(() => parseStructuredLifecycleIntentV1({ ...intent, owner: payer }))
      .toThrow('Structured intent has missing, unknown or invalid fields');
    expect(() => structuredLifecycleIntentV1({ market, payer, action: 'transfer', coordinate: '' }))
      .toThrow('Choose a supported Structured lifecycle action');
  });

  it('rejects completion of an operation started before a Market or action edit', () => {
    const selection = new StructuredLifecycleSelectionV1();
    const first = selection.current();
    selection.require(first);
    selection.invalidate();
    expect(() => selection.require(first)).toThrow('Structured selection changed; review the current Market and action');
    selection.require(selection.current());
  });
});
