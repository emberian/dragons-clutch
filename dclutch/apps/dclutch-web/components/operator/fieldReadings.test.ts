import { describe, expect, it } from 'vitest';
import { PublicKey } from '@solana/web3.js';

import {
  compactAddressV1,
  FieldRefusalV1,
  readAtomsV1,
  readEnumV1,
  readEvidenceV1,
  readEndpointV1,
  readHex64V1,
  readPubkeyV1,
  readU64V1,
  type EvidenceReadingV1,
  type FieldReadingV1,
} from './fieldReadings';
import type { DenominationV1 } from '@/lib/quantity';

function key(seed: number): string {
  return new PublicKey(new Uint8Array(32).fill(seed)).toBase58();
}

const SIX_DECIMALS: DenominationV1 = { decimals: 6, unit: null, mint: key(9) };
const UNREAD: DenominationV1 = { decimals: null, unit: null, mint: key(9) };

/**
 * Every refusal in this module is held to the same grammar, in one place, so a
 * new field type cannot quietly ship a worse one.
 *
 * The bar is `lib/tradeFlowRefusals.ts`'s: the remedy is ONE imperative
 * sentence saying what to do, and the detail is a separate sentence saying
 * what the field actually found. A remedy that opens by describing the problem
 * has put the cause first, which is the ordering this whole redesign exists to
 * invert.
 */
function refusalOf(reading: FieldReadingV1 | EvidenceReadingV1): Readonly<{ remedy: string; detail: string }> {
  if (reading.state !== 'refused') throw new Error(`expected a refusal, got ${reading.state}`);
  const { remedy, detail } = reading;

  // One sentence, and it ends like one.
  expect(remedy.endsWith('.')).toBe(true);
  expect(remedy.split('. ').length).toBe(1);
  // Imperative: it opens with a verb the reader can act on, not with "This".
  expect(/^(Paste|Enter|Choose|Connect|Derive|Load|Fix|Use|Read)\b/.test(remedy)).toBe(true);
  // The two halves are distinct; the detail is not a restatement of the remedy.
  expect(detail).not.toBe(remedy);
  expect(detail.length).toBeGreaterThan(0);
  return { remedy, detail };
}

describe('readPubkeyV1', () => {
  it('reads nothing from nothing, rather than refusing an untouched field', () => {
    expect(readPubkeyV1('')).toEqual({ state: 'empty' });
    expect(readPubkeyV1('   ')).toEqual({ state: 'empty' });
  });

  it('resolves a real address to its byte width and a compact spelling', () => {
    const reading = readPubkeyV1(key(3));
    if (reading.state !== 'resolved') throw new Error('expected a resolved reading');
    expect(reading.identity).toContain('32 bytes');
    expect(reading.identity).toContain(compactAddressV1(key(3)));
  });

  it('names the account when the page can identify it', () => {
    const reading = readPubkeyV1(key(3), (address) => address === key(3) ? 'the activation cache derived in step 02' : null);
    if (reading.state !== 'resolved') throw new Error('expected a resolved reading');
    expect(reading.identity).toContain('the activation cache derived in step 02');
  });

  it('refuses a base58 stray by naming the character and its position', () => {
    // `0` is the canonical base58 omission: it is confusable with `O`.
    const { remedy, detail } = refusalOf(readPubkeyV1(`${key(3).slice(0, 7)}0${key(3).slice(8)}`));
    expect(remedy).toBe('Paste the address as base58.');
    expect(detail).toContain('Character 8');
    expect(detail).toContain('"0"');
    expect(detail).toContain('omits 0, O, I and l');
  });

  it('refuses a well-spelled address of the wrong byte width, and counts it', () => {
    const { remedy, detail } = refusalOf(readPubkeyV1('abcdef'));
    expect(remedy).toBe('Paste a 32-byte account address.');
    expect(detail).toContain('6 base58 characters');
  });

  it('replaces the library throw that /workbench and /operate show today', () => {
    // Measured at HEAD: `canonicalKey` (lib/operatorSurface.ts) calls
    // `new PublicKey` unguarded, so a mistyped program address reaches the
    // reader as web3.js's own words. This field never emits either string.
    const { remedy, detail } = refusalOf(readPubkeyV1('not a real address at all'));
    for (const libraryWords of ['Invalid public key input', 'Non-base58 character']) {
      expect(remedy).not.toContain(libraryWords);
      expect(detail).not.toContain(libraryWords);
    }
  });

  it('needs no canonical-spelling refusal, because 32-byte base58 admits one spelling', () => {
    // The reason `readPubkeyV1` carries no round-trip refusal, pinned as a
    // property rather than asserted in a comment. A leading `1` is a base58
    // zero byte, so the obvious second spelling of a key is a 33-byte string
    // and never decodes; and every address that DOES decode round-trips.
    const systemProgram = new PublicKey(new Uint8Array(32)).toBase58();
    expect(() => new PublicKey(`1${systemProgram}`)).toThrow();
    for (const seed of [0, 1, 7, 42, 255]) {
      const address = new PublicKey(new Uint8Array(32).fill(seed)).toBase58();
      expect(new PublicKey(address).toBase58()).toBe(address);
      expect(readPubkeyV1(address).state).toBe('resolved');
    }
  });
});

describe('readHex64V1', () => {
  it('resolves a 64-character digest', () => {
    const reading = readHex64V1('a'.repeat(64));
    if (reading.state !== 'resolved') throw new Error('expected a resolved reading');
    expect(reading.identity).toContain('32 bytes');
    expect(reading.identity).toContain('aaaaaaaa…aaaaaaaa');
  });

  it('tells a digest field holding an address which of the two it has', () => {
    // The malformation this type exists to catch: /product-v2 puts three hex64
    // digests and eight pubkeys on one form, and today neither kind of field
    // can tell you it is holding the other kind.
    const { remedy, detail } = refusalOf(readHex64V1(key(5)));
    expect(remedy).toBe('Paste the 64-character hex digest.');
    expect(detail).toContain('base58 address, which names an account rather than a digest');
  });

  it('counts how far a short digest falls from 64', () => {
    const { remedy, detail } = refusalOf(readHex64V1('ab'.repeat(31)));
    expect(remedy).toBe('Paste all 64 hex characters.');
    expect(detail).toContain('This is 62');
    expect(detail).toContain('2 short of');
  });

  it('names a stray character that is neither hex nor base58', () => {
    const { detail } = refusalOf(readHex64V1(`${'a'.repeat(63)}!`));
    expect(detail).toContain('Character 64');
    expect(detail).toContain('0-9 and the letters a-f');
  });
});

describe('readAtomsV1', () => {
  it('reports the exact atoms first and the humanized amount second', () => {
    const reading = readAtomsV1('500000000', SIX_DECIMALS);
    if (reading.state !== 'resolved') throw new Error('expected a resolved reading');
    // FLOWFUL_IA_V1 §5.4: on an operator surface the atom count leads.
    expect(reading.identity.startsWith('500000000 atoms')).toBe(true);
    expect(reading.identity).toContain('500 collateral at 6 decimals');
  });

  it('says so, rather than guessing, when the mint published no precision', () => {
    const reading = readAtomsV1('500000000', UNREAD);
    if (reading.state !== 'resolved') throw new Error('expected a resolved reading');
    expect(reading.identity).toContain('500000000 atoms');
    expect(reading.identity).toContain('never published a display precision');
    // No invented ticker, and no silent treatment of unknown decimals as zero.
    expect(reading.identity).toContain('collateral');
  });

  it('refuses a decimal point, because an atom does not divide', () => {
    const { remedy, detail } = refusalOf(readAtomsV1('1.5', SIX_DECIMALS));
    expect(remedy).toBe('Enter this amount in whole atoms.');
    expect(detail).toContain('does not divide');
  });

  it('refuses a bare decimal fragment the same way', () => {
    expect(refusalOf(readAtomsV1('.5', SIX_DECIMALS)).remedy).toBe('Enter this amount in whole atoms.');
  });

  it('refuses a negative amount by naming the unsigned wire', () => {
    expect(refusalOf(readAtomsV1('-1', SIX_DECIMALS)).detail).toContain('unsigned integers');
  });

  it('refuses zero as a slot spent to move nothing', () => {
    expect(refusalOf(readAtomsV1('0', SIX_DECIMALS)).detail).toContain('spends a slot and a signature, and moves nothing');
  });

  it('refuses past u64 and names the bound', () => {
    expect(refusalOf(readAtomsV1('18446744073709551616', SIX_DECIMALS)).detail)
      .toContain('18,446,744,073,709,551,615');
  });

  it('accepts u64 max itself, so the bound is exclusive of nothing', () => {
    expect(readAtomsV1('18446744073709551615', SIX_DECIMALS).state).toBe('resolved');
  });

  it('accepts the grouping separators the site itself prints', () => {
    expect(readAtomsV1('500,000,000', SIX_DECIMALS).state).toBe('resolved');
  });

  it('refuses a unit suffix rather than silently reading the digits', () => {
    expect(refusalOf(readAtomsV1('500 atoms', SIX_DECIMALS)).remedy).toBe('Enter the amount as digits only.');
  });
});

describe('readEvidenceV1', () => {
  const summarize = (parsed: unknown) => {
    const record = parsed as Record<string, unknown>;
    if (typeof record.market !== 'string') {
      throw new FieldRefusalV1(
        'Paste the founding packet this console asks for.',
        'This parses as JSON but names no market, so it is some other artifact.',
      );
    }
    return { identity: 'founding packet', rows: [{ term: 'market', detail: record.market }] };
  };

  it('summarizes a valid artifact instead of leaving a blob on screen', () => {
    const reading = readEvidenceV1('{"market":"abc"}', summarize);
    if (reading.state !== 'resolved') throw new Error('expected a resolved reading');
    expect(reading.identity).toBe('founding packet');
    expect(reading.rows).toEqual([{ term: 'market', detail: 'abc' }]);
  });

  it('refuses a truncated paste by asking for the whole file', () => {
    const { remedy, detail } = refusalOf(readEvidenceV1('{"market":', summarize));
    expect(remedy).toContain('Paste the whole file');
    expect(detail).toContain('not complete JSON yet');
  });

  it("carries the console's own two-part refusal through unchanged", () => {
    const { remedy, detail } = refusalOf(readEvidenceV1('{"unrelated":1}', summarize));
    expect(remedy).toBe('Paste the founding packet this console asks for.');
    expect(detail).toBe('This parses as JSON but names no market, so it is some other artifact.');
  });

  it('still refuses readably when a summarizer throws a plain Error', () => {
    const { remedy, detail } = refusalOf(readEvidenceV1('{}', () => { throw new Error('no schema field'); }));
    expect(remedy).toBe('Paste the artifact this field asks for.');
    expect(detail).toBe('no schema field');
  });
});

describe('readEnumV1', () => {
  it('resolves a known choice', () => {
    expect(readEnumV1('core', ['core', 'registry'])).toEqual({ state: 'resolved', identity: 'core' });
  });

  it('refuses an unknown choice by listing the ones that exist', () => {
    const { remedy, detail } = refusalOf(readEnumV1('cor', ['core', 'registry']));
    expect(remedy).toBe('Choose one of core, registry.');
    expect(detail).toContain('"cor"');
  });
});

describe('readU64V1', () => {
  it('reports the exact integer first and a grouped reading second', () => {
    const reading = readU64V1('1400000', { noun: 'compute units' });
    if (reading.state !== 'resolved') throw new Error('expected a resolved reading');
    expect(reading.identity.startsWith('1400000 ·')).toBe(true);
    expect(reading.identity).toContain('1,400,000 compute units');
  });

  it('refuses a value outside the range and names both ends', () => {
    const { remedy, detail } = refusalOf(readU64V1('1400001', { noun: 'compute units', min: 1n, max: 1_400_000n }));
    expect(remedy).toContain('within 1 to 1,400,000');
    expect(detail).toContain('above the range');
  });

  it('refuses a decimal, which inputMode never did', () => {
    expect(refusalOf(readU64V1('1.5', { noun: 'generation' })).remedy).toBe('Enter the generation as digits only.');
  });

  it('refuses a negative by naming the unsigned wire', () => {
    expect(refusalOf(readU64V1('-3', { noun: 'generation' })).remedy).toBe('Enter a positive generation.');
  });

  it('names the noun in the refusal, so the message is about this field', () => {
    expect(refusalOf(readU64V1('x', { noun: 'outcome index' })).remedy).toContain('outcome index');
  });
});

describe('readEndpointV1', () => {
  it('resolves an endpoint to its scheme and host', () => {
    const reading = readEndpointV1('http://127.0.0.1:20890');
    if (reading.state !== 'resolved') throw new Error('expected a resolved reading');
    expect(reading.identity).toBe('http · 127.0.0.1:20890');
  });

  it('replaces the Invalid URL the consoles show today, and shows an example', () => {
    const { remedy, detail } = refusalOf(readEndpointV1('127.0.0.1:20890'));
    expect(remedy).toBe('Enter the endpoint as a full URL, scheme included.');
    expect(detail).toContain('http://127.0.0.1:20890');
    expect(remedy).not.toContain('Invalid URL');
    expect(detail).not.toContain('Invalid URL');
  });

  it('refuses a non-http scheme by naming the scheme it got', () => {
    const { remedy, detail } = refusalOf(readEndpointV1('ws://127.0.0.1:20890'));
    expect(remedy).toBe('Enter an http or https endpoint.');
    expect(detail).toContain('This one is ws');
  });
});
