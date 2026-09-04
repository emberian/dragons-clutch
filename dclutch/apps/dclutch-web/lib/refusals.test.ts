import { describe, expect, it } from 'vitest';

import { REFUSAL_BANDS_V1, REFUSAL_BAND_SPAN } from './generated/refusalBandsV1';
import { REFUSAL_CODES_V1 } from './generated/refusalRegistryV1';
import { customCodeFromTransactionError, refusalBand, refusalCode, releaseSupersededMeaningV1, renderRefusal } from './refusals';

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

describe('the supersession story decision 0012 registered', () => {
  it('is one meaning carried by exactly the eight reader entries, so no client invents its own', () => {
    const rows = REFUSAL_CODES_V1.filter((entry) => entry.name.endsWith('::ReleaseSuperseded'));
    // Decision 0012 gave one discriminant per reader. It was eight until
    // `dclutch-dealer-sbf` was deleted on 2026-09-02 and band 7 retired (which
    // took `dealer 0x700A` out of refusals.md), seven until 2026-09-03, when
    // the Claims founding route's one coarse `Release` became named accusations
    // (1b4e5d310) and gained its own `ReleaseSuperseded` at 0x5190 — a second
    // entry in the claims band, because that program carries two entry
    // families. If a reader ever grows one, this list moves deliberately rather
    // than by accident.
    expect(rows.map((entry) => entry.code)).toEqual([0x100D, 0x200B, 0x3010, 0x4007, 0x500A, 0x5190, 0x600C, 0x8014]);
    expect(rows.map((entry) => entry.band)).toEqual(['registry', 'rent', 'core', 'trading', 'claims', 'claims', 'custody', 'resolution']);
    expect(new Set(rows.map((entry) => entry.meaning)).size).toBe(1);
  });

  it('says the pin moved and names the remedy, which is what a browser must repeat verbatim', () => {
    const meaning = releaseSupersededMeaningV1();
    expect(meaning).toContain('the substrate was upgraded');
    // The remedy half. A refusal a reader cannot act on is a mystery, and the
    // registry is where the action lives.
    expect(meaning).toContain('re-release re-authenticates the new deployment and re-pins its slot');
  });

  it('is the refusal, not the reasoning behind it', () => {
    // Both halves above are facts about the deployment the reader is holding.
    // The decision reference and the it-is-not-an-attack reassurance are for
    // someone reading the enum, so they stay in the doc comment's second
    // paragraph and out of every caption generated from it.
    const meaning = releaseSupersededMeaningV1();
    expect(meaning).not.toContain('Decision 0012');
    expect(meaning).not.toContain('not an attack');
    expect(meaning.split('. ').length).toBeLessThanOrEqual(2);
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
