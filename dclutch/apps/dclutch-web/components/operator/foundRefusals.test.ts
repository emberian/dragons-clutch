import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

import {
  assignFoundRefusalV1,
  FOUND_RAW_RECORD_ORDER_V1,
  FOUND_UNROUTED_BY_DESIGN_V1,
  routedFoundFragmentsV1,
} from './foundRefusals';

const ROOT = join(import.meta.dirname, '..', '..');
const SOURCES = ['lib/coreFound.ts', 'lib/infrastructure.ts', 'lib/rpc.ts', 'components/CoreFoundWorkspace.tsx']
  .map((path) => readFileSync(join(ROOT, path), 'utf8'))
  .join('\n');

/** Produced by the platform, not by this project — nothing to find in source. */
const PLATFORM_FRAGMENTS = new Set(['Invalid URL']);


/**
 * True when `fragment` is `<label> <template>` and BOTH halves are real: the
 * template appears in source as a thrown string, and the label appears as a
 * quoted argument passed to the validator that throws it.
 *
 * Checking only the template would let a wrong label through -- which is the
 * exact bug this suite already caught once, where the table carried
 * `/release`'s "wrong owner" wording for a `/found` field that says
 * "wrong Registry owner".
 */
function splitsIntoLabelAndTemplate(fragment: string): boolean {
  const words = fragment.split(' ');
  for (let cut = 1; cut < words.length; cut += 1) {
    const label = words.slice(0, cut).join(' ');
    const template = words.slice(cut).join(' ');
    if (SOURCES.includes(`\`${'${'}field} ${template}\``) && SOURCES.includes(`'${label}'`)) return true;
  }
  return false;
}

describe('the /found routing table matches the refusals that actually exist', () => {
  it('routes only fragments the source really produces', () => {
    // The guard that stops this table from rotting into fiction. A refusal
    // that gets reworded upstream must fail here, not silently stop routing.
    const missing: string[] = [];
    for (const fragment of routedFoundFragmentsV1()) {
      if (PLATFORM_FRAGMENTS.has(fragment)) continue;
      // The positional refusals are built from a template, so the source
      // carries the prefix and an interpolation rather than the literal.
      if (/^finalized raw record \d+$/.test(fragment)) {
        if (!SOURCES.includes('finalized raw record ')) missing.push(fragment);
        continue;
      }
      if (SOURCES.includes(fragment)) continue;
      // Many refusals are `${field} <template>`, so neither half is literal on
      // its own. Both halves must exist: the template as thrown, and the label
      // as it is actually passed to the validator.
      if (splitsIntoLabelAndTemplate(fragment)) continue;
      missing.push(fragment);
    }
    expect(missing).toEqual([]);
  });

  it('leaves the join refusals unrouted, and they are real refusals too', () => {
    for (const fragment of FOUND_UNROUTED_BY_DESIGN_V1) {
      expect(SOURCES).toContain(fragment);
      const routed = assignFoundRefusalV1(fragment);
      expect(routed.routed).toBe(false);
      expect(routed.field).toBeNull();
    }
  });

  it('reads the ten records in the order prepareCoreFoundV2 reads them', () => {
    // `lib/coreFound.ts` builds `rawAddresses` as one array literal and
    // validates it by index. If that order changes, every positional refusal
    // in this table starts pointing at the wrong field, silently.
    const literal = SOURCES.slice(SOURCES.indexOf('const rawAddresses = ['));
    const order = literal.slice(0, literal.indexOf(']'));
    let cursor = -1;
    for (const record of FOUND_RAW_RECORD_ORDER_V1) {
      const at = order.indexOf(`input.${record.field}`);
      expect(at, `${record.field} missing from rawAddresses`).toBeGreaterThan(-1);
      expect(at, `${record.field} out of order`).toBeGreaterThan(cursor);
      cursor = at;
    }
  });
});

describe('assignFoundRefusalV1', () => {
  it('turns a positional refusal into the label on the screen', () => {
    // The defect this whole module exists for: the SDK says "record 4", the
    // screen says "Linked basis raw", and nothing connected them.
    const routed = assignFoundRefusalV1('finalized raw record 4 must be canonical base58 text');
    expect(routed.routed).toBe(true);
    expect(routed.field).toBe('linkedBasisRecord');
    expect(routed.remedy).toBe('Check the address in Linked basis raw.');
    // The refusal's own words survive whole.
    expect(routed.detail).toBe('finalized raw record 4 must be canonical base58 text');
  });

  it('routes every one of the ten positions to its own field', () => {
    const fields = FOUND_RAW_RECORD_ORDER_V1.map((_, index) =>
      assignFoundRefusalV1(`finalized raw record ${index} must be canonical base58 text`).field);
    expect(fields).toEqual(FOUND_RAW_RECORD_ORDER_V1.map((record) => record.field));
    expect(new Set(fields).size).toBe(10);
  });

  it('gives the unauthored URL throw a remedy and an owner', () => {
    const routed = assignFoundRefusalV1('Refused: Invalid URL');
    expect(routed.field).toBe('endpoint');
    expect(routed.remedy).toBe('Enter the endpoint as a full URL, scheme included.');
  });

  it('prefers the more specific fragment when two could match', () => {
    // "activation cache must be canonical base58 text" and the PDA refusal
    // both mention the activation cache; ordering decides, and both land on
    // the same field, so the remedy is what differs.
    expect(assignFoundRefusalV1('activation cache is not the release-derived Registry PDA').remedy)
      .toBe('Use the activation cache this release derives.');
    expect(assignFoundRefusalV1('activation cache must be canonical base58 text').remedy)
      .toBe('Paste the activation cache address as base58.');
  });

  it('keeps an unknown refusal whole, at form level, and says it is unrouted', () => {
    const routed = assignFoundRefusalV1('something nobody has seen before');
    expect(routed.routed).toBe(false);
    expect(routed.field).toBeNull();
    expect(routed.detail).toBe('something nobody has seen before');
    expect(routed.remedy).toBe('This construction refused. Its own words are below.');
  });

  it('never paraphrases the detail, whatever it routes', () => {
    for (const fragment of [...routedFoundFragmentsV1(), 'unknown']) {
      expect(assignFoundRefusalV1(`Refused: ${fragment} (extra)`).detail)
        .toBe(`Refused: ${fragment} (extra)`);
    }
  });

  it('gives every routed refusal an imperative remedy', () => {
    for (const fragment of routedFoundFragmentsV1()) {
      const { remedy } = assignFoundRefusalV1(fragment);
      expect(remedy.endsWith('.')).toBe(true);
      expect(/^(Check|Enter|Paste|Use|Point|Choose)\b/.test(remedy), `${fragment} -> ${remedy}`).toBe(true);
    }
  });
});
