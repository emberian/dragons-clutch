import { describe, expect, it } from 'vitest';

import { REFUSAL_BANDS_V1, REFUSAL_BAND_SPAN, REFUSAL_CODES_V1 } from './generated/refusalRegistryV1';
import { customCodeFromTransactionError, refusalBand, refusalCode, renderRefusal } from './refusals';

describe('refusal band arithmetic', () => {
  it('never allocates band 0, so a foreign code below 0x1000 is provably not ours', () => {
    for (const band of REFUSAL_BANDS_V1) expect(band.base).toBeGreaterThanOrEqual(0x1000);
    expect(refusalBand(0)).toBeNull();
    expect(refusalBand(0x0fff)).toBeNull();
    expect(renderRefusal(3).origin).toBe('foreign');
    // SPL Token's InsufficientFunds is Custom(1); it must never render as dClutch.
    expect(renderRefusal(1).text).toContain('not a dClutch refusal');
  });

  it('keeps the band table ascending and disjoint, as the Rust const assertions do', () => {
    for (let index = 1; index < REFUSAL_BANDS_V1.length; index += 1) {
      const previous = REFUSAL_BANDS_V1[index - 1];
      const entry = REFUSAL_BANDS_V1[index];
      expect(previous).toBeDefined();
      expect(entry).toBeDefined();
      if (previous === undefined || entry === undefined) throw new Error('unreachable');
      expect(entry.base).toBeGreaterThanOrEqual(previous.base + REFUSAL_BAND_SPAN);
    }
  });

  it('owns every registered code with the band its section names', () => {
    expect(REFUSAL_CODES_V1.length).toBeGreaterThan(150);
    for (const entry of REFUSAL_CODES_V1) {
      const band = refusalBand(entry.code);
      expect(band, `code 0x${entry.code.toString(16)} has no band`).not.toBeNull();
      if (band === null) throw new Error('unreachable');
      expect(band.label).toBe(entry.band);
    }
  });

  it('renders a registered refusal with its program, name, and meaning', () => {
    const first = REFUSAL_CODES_V1[0];
    expect(first).toBeDefined();
    if (first === undefined) throw new Error('unreachable');
    const rendered = renderRefusal(first.code);
    expect(rendered.origin).toBe('first-party');
    expect(rendered.text).toContain(first.name);
    expect(rendered.text).toContain(first.meaning);
  });

  it('renders an in-band unregistered code as the program plus the bare code, never a guessed name', () => {
    const band = REFUSAL_BANDS_V1[0];
    expect(band).toBeDefined();
    if (band === undefined) throw new Error('unreachable');
    // The top of a band is a legal code the reference is unlikely to register.
    const code = band.base + REFUSAL_BAND_SPAN - 1;
    expect(refusalCode(code)).toBeNull();
    const rendered = renderRefusal(code);
    expect(rendered.origin).toBe('first-party');
    expect(rendered.text).toContain(band.label);
    expect(rendered.text).toContain('no row');
  });
});

describe('transaction error extraction', () => {
  it('pulls the custom code out of the JSON-RPC InstructionError shape', () => {
    expect(customCodeFromTransactionError({ InstructionError: [0, { Custom: 0x5000 }] })).toBe(0x5000);
  });

  it('returns null for every other failure kind rather than inventing a code', () => {
    expect(customCodeFromTransactionError(null)).toBeNull();
    expect(customCodeFromTransactionError('AccountNotFound')).toBeNull();
    expect(customCodeFromTransactionError({ InstructionError: [0, 'InvalidAccountData'] })).toBeNull();
    expect(customCodeFromTransactionError({ InstructionError: [0, { Custom: -1 }] })).toBeNull();
    expect(customCodeFromTransactionError({ InstructionError: [0] })).toBeNull();
  });
});
