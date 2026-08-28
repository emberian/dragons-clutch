import { describe, expect, it } from 'vitest';

import {
  RESOLUTION_CERTIFICATE_V2_REFUSAL_CORPUS_HEX,
  RESOLUTION_CERTIFICATE_V2_WIDE_FAILURE_EXAMPLE_HEX,
  RESOLUTION_CERTIFICATE_V2_WIDE_SUCCESS_EXAMPLE_HEX,
} from './generated/resolutionCertificateV2';
import { bindTerminalResolutionCertificateV2, decodeResolutionCertificateV2 } from './resolutionCertificateV2';

function bytes(value: string): Uint8Array {
  return Uint8Array.from(value.match(/../g) ?? [], (byte) => Number.parseInt(byte, 16));
}

describe('ResolutionCertificateV2', () => {
  it('preserves the canonical i128/u64 result and native u32 selector', () => {
    const certificate = decodeResolutionCertificateV2(bytes(RESOLUTION_CERTIFICATE_V2_WIDE_SUCCESS_EXAMPLE_HEX));
    expect(certificate).toMatchObject({
      kind: 'resolution-success',
      generation: 9n,
      selector: 257,
      resultNumerator: 7n,
      resultDenominator: 1n,
      observedAt: 100n,
    });
    expect(bindTerminalResolutionCertificateV2(certificate, {
      receiptAccount: certificate.receiptAccount,
      market: certificate.market,
      sourceMaterial: certificate.sourceMaterial,
      productRecordDigest: certificate.productRecordDigest,
      generation: 9n,
      selector: 257,
      outcomeCount: 259,
    })).toBe(certificate);
  });

  it('admits only the final Product cell for an explicit failure certificate', () => {
    const certificate = decodeResolutionCertificateV2(bytes(RESOLUTION_CERTIFICATE_V2_WIDE_FAILURE_EXAMPLE_HEX));
    expect(certificate).toMatchObject({ kind: 'resolution-failure', selector: 257, resultDenominator: 0n });
    expect(() => bindTerminalResolutionCertificateV2(certificate, {
      receiptAccount: certificate.receiptAccount,
      market: certificate.market,
      sourceMaterial: certificate.sourceMaterial,
      productRecordDigest: certificate.productRecordDigest,
      generation: certificate.generation,
      selector: certificate.selector,
      outcomeCount: 259,
    })).toThrow('kind and selector');
  });

  it('refuses every hostile byte string emitted with the canonical Rust codec', () => {
    expect(RESOLUTION_CERTIFICATE_V2_REFUSAL_CORPUS_HEX).toHaveLength(13);
    for (const hostile of RESOLUTION_CERTIFICATE_V2_REFUSAL_CORPUS_HEX) {
      expect(() => decodeResolutionCertificateV2(bytes(hostile))).toThrow();
    }
  });

  it('refuses substituted Core authority at every persisted join', () => {
    const certificate = decodeResolutionCertificateV2(bytes(RESOLUTION_CERTIFICATE_V2_WIDE_SUCCESS_EXAMPLE_HEX));
    const base = {
      receiptAccount: certificate.receiptAccount,
      market: certificate.market,
      sourceMaterial: certificate.sourceMaterial,
      productRecordDigest: certificate.productRecordDigest,
      generation: certificate.generation,
      selector: certificate.selector,
      outcomeCount: 259,
    };
    for (const field of ['receiptAccount', 'market', 'sourceMaterial', 'productRecordDigest'] as const) {
      const substituted = new Uint8Array(base[field]); substituted[31] ^= 1;
      expect(() => bindTerminalResolutionCertificateV2(certificate, { ...base, [field]: substituted })).toThrow('Core terminal authority');
    }
    expect(() => bindTerminalResolutionCertificateV2(certificate, { ...base, generation: base.generation + 1n })).toThrow('Core terminal authority');
    expect(() => bindTerminalResolutionCertificateV2(certificate, { ...base, selector: base.selector + 1 })).toThrow('Core terminal authority');
  });
});
