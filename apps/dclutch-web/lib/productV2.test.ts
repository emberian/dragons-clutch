import { AddressLookupTableAccount, PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  PAYOFF_ADMISSION_REQUEST_BYTES_V1,
  PAYOFF_REQUEST_BYTES_V2,
  PRODUCT_EVALUATOR_ACCOUNT_COUNT,
  PRODUCT_LIABILITY_ADMISSION_ACCOUNT_COUNT,
  PRODUCT_V2_BYTES,
  compileProductV2,
  compileProductV2LiabilityTransaction,
  evaluateProductV2,
  parseProductKnots,
  parseProductTerms,
} from './productV2';

function key(seed: number): string { return new PublicKey(new Uint8Array(32).fill(seed)).toBase58(); }

async function fixture() {
  return compileProductV2({
    productId: 41n,
    domainId: 42n,
    coordinateUnitId: 43n,
    payoutScale: 1_000_000n,
    knotDenominator: 2n,
    knots: [-100n, 0n, 100n],
    terms: [
      { shape: 'tent', left: 0, peak: 1, right: 2, amplitude: 50n },
      { shape: 'constant', left: 0, peak: 0, right: 0, amplitude: 3n },
      { shape: 'ramp-up', left: 0, peak: 0, right: 1, amplitude: 100n },
    ],
  });
}

describe('Product V2 exact signed-rational studio', () => {
  it('canonicalizes runtime-width terms into one exact 576-byte content identity', async () => {
    const product = await fixture();
    expect(product.bytes).toHaveLength(PRODUCT_V2_BYTES);
    expect(new TextDecoder().decode(product.bytes.slice(0, 8))).toBe('DCLTPAY2');
    expect(product.bytes[10]).toBe(3);
    expect(product.bytes[11]).toBe(3);
    expect(Array.from([product.bytes[320], product.bytes[336], product.bytes[352]])).toEqual([0, 1, 3]);
    expect(product.input.terms.map((term) => term.shape)).toEqual(['constant', 'ramp-up', 'tent']);
    expect(product.liabilityBound).toBe(153n);
    expect(product.digestHex).toMatch(/^[0-9a-f]{64}$/);
    expect(product.regions.map((region) => [region.left, region.right])).toEqual([
      ['−∞', '-100/2'], ['-100/2', '0/2'], ['0/2', '100/2'], ['100/2', '+∞'],
    ]);
  });

  it('keeps the coordinate rational and floors only each final interpolation contribution', async () => {
    const product = await fixture();
    expect(evaluateProductV2(product, -50n, 1n)).toBe(3n);
    expect(evaluateProductV2(product, -25n, 1n)).toBe(78n);
    expect(evaluateProductV2(product, 0n, 7n)).toBe(153n);
    expect(evaluateProductV2(product, 25n, 1n)).toBe(128n);
    expect(evaluateProductV2(product, 1n, 3n)).toBe(152n);
    expect(evaluateProductV2(product, 10_000n, 1n)).toBe(103n);
  });

  it('refuses noncanonical integers, partitions, terms, and arithmetic bounds', async () => {
    expect(() => parseProductKnots('01\n2')).toThrow('canonical');
    expect(() => parseProductTerms('ramp-up 0  1 50')).toThrow('canonical single spaces');
    await expect(compileProductV2({
      productId: 1n, domainId: 2n, coordinateUnitId: 3n, payoutScale: 4n, knotDenominator: 1n,
      knots: [0n, 0n], terms: [{ shape: 'constant', left: 0, peak: 0, right: 0, amplitude: 1n }],
    })).rejects.toThrow('strictly increasing');
    await expect(compileProductV2({
      productId: 1n, domainId: 2n, coordinateUnitId: 3n, payoutScale: 4n, knotDenominator: 1n,
      knots: [0n, 1n], terms: [
        { shape: 'ramp-up', left: 0, peak: 0, right: 1, amplitude: 1n },
        { shape: 'ramp-up', left: 0, peak: 0, right: 1, amplitude: 2n },
      ],
    })).rejects.toThrow('duplicate');
    const product = await fixture();
    expect(() => evaluateProductV2(product, 0n, 0n)).toThrow('denominator');
    expect(() => evaluateProductV2(product, 1n << 127n, 1n)).toThrow('i128');
  });

  it('builds one unsigned packet-bounded 10-account evidence plus 28-account admission transaction', () => {
    const payer = key(1); const evaluatorProgram = key(2); const admissionProgram = key(3);
    const evaluatorAccounts = [payer, ...Array.from({ length: PRODUCT_EVALUATOR_ACCOUNT_COUNT - 1 }, (_, index) => key(4 + index))];
    const admissionAccounts = [payer, ...Array.from({ length: PRODUCT_LIABILITY_ADMISSION_ACCOUNT_COUNT - 1 }, (_, index) => key(20 + index))];
    const lookupTable = new AddressLookupTableAccount({
      key: new PublicKey(key(90)),
      state: {
        deactivationSlot: 18_446_744_073_709_551_615n,
        lastExtendedSlot: 77,
        lastExtendedSlotStartIndex: 0,
        authority: new PublicKey(key(91)),
        addresses: [...evaluatorAccounts.slice(1), ...admissionAccounts.slice(1)].map((address) => new PublicKey(address)),
      },
    });
    const compiled = compileProductV2LiabilityTransaction({
      payer,
      recentBlockhash: key(92),
      computeUnitLimit: 900_000,
      lookupTable,
      request: new Uint8Array(PAYOFF_REQUEST_BYTES_V2),
      admissionRequest: new Uint8Array(PAYOFF_ADMISSION_REQUEST_BYTES_V1),
      evaluatorProgram,
      admissionProgram,
      evaluatorAccounts,
      admissionAccounts,
    });
    expect(compiled.wireBytes.length).toBeLessThanOrEqual(1_232);
    expect(compiled.requiredSigners).toEqual([payer]);
    expect(compiled.lookupAddressesUsed).toBe((PRODUCT_EVALUATOR_ACCOUNT_COUNT - 1) + (PRODUCT_LIABILITY_ADMISSION_ACCOUNT_COUNT - 1));
    expect(compiled.transaction.message.compiledInstructions).toHaveLength(3);
    expect(compiled.transaction.message.compiledInstructions[1].accountKeyIndexes).toHaveLength(PRODUCT_EVALUATOR_ACCOUNT_COUNT);
    expect(compiled.transaction.message.compiledInstructions[2].accountKeyIndexes).toHaveLength(PRODUCT_LIABILITY_ADMISSION_ACCOUNT_COUNT);
  });

  it('refuses aliased or deactivated transaction authority', () => {
    const payer = key(1); const evaluatorAccounts = [payer, ...Array.from({ length: PRODUCT_EVALUATOR_ACCOUNT_COUNT - 1 }, (_, index) => key(4 + index))]; const admissionAccounts = [payer, ...Array.from({ length: PRODUCT_LIABILITY_ADMISSION_ACCOUNT_COUNT - 1 }, (_, index) => key(20 + index))];
    const transaction = (deactivationSlot: bigint, accounts = evaluatorAccounts) => compileProductV2LiabilityTransaction({
      payer, recentBlockhash: key(92), computeUnitLimit: 1, evaluatorProgram: key(2), admissionProgram: key(3), evaluatorAccounts: accounts, admissionAccounts,
      request: new Uint8Array(PAYOFF_REQUEST_BYTES_V2), admissionRequest: new Uint8Array(PAYOFF_ADMISSION_REQUEST_BYTES_V1),
      lookupTable: new AddressLookupTableAccount({ key: new PublicKey(key(90)), state: { deactivationSlot, lastExtendedSlot: 1, lastExtendedSlotStartIndex: 0, authority: undefined, addresses: [] } }),
    });
    expect(() => transaction(18_446_744_073_709_551_615n, [payer, payer, ...evaluatorAccounts.slice(2)])).toThrow('aliases');
    expect(() => transaction(0n)).toThrow('deactivated');
  });
});
