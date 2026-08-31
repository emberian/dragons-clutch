import { describe, expect, it } from 'vitest';

import { PROTOCOL_REFUSALS, REFUSAL_BANDS } from '../generated/routeCensus';
import {
  attributeCustomCode,
  attributionTitle,
  bandForCode,
  customCodeFromError,
  describeAttribution,
  invokedFrames,
  readReportedRefusal,
  runtimeErrorLabel,
} from './refusals';

const CORE = REFUSAL_BANDS.find((band) => band.label === 'core');
const CLAIMS = REFUSAL_BANDS.find((band) => band.label === 'claims');
const TEST_CALLER = REFUSAL_BANDS.find((band) => band.tier === 'test-caller');

describe('band attribution', () => {
  it('refuses to claim anything below one band width', () => {
    // Decision 0007's load-bearing property: SPL Token and the loader number
    // from zero, and now nothing of ours does.
    for (const code of [0, 1, 6, 0x0fff]) {
      const attribution = attributeCustomCode(code);
      expect(attribution.disposition).toBe('foreign');
      expect(attributionTitle(attribution)).toBe('not a dClutch refusal');
      expect(bandForCode(code)).toBeNull();
    }
  });

  it('names a code the census enumerates, with its own doc comment', () => {
    const refusal = PROTOCOL_REFUSALS.find((entry) => entry.program === 'core');
    expect(refusal).toBeDefined();
    if (refusal === undefined) return;
    const attribution = attributeCustomCode(refusal.code);
    expect(attribution.disposition).toBe('named');
    if (attribution.disposition !== 'named') return;
    expect(attribution.refusal.id).toBe(refusal.id);
    expect(attributionTitle(attribution)).toBe(`${refusal.enumName}::${refusal.variant}`);
    expect(describeAttribution(attribution)).toBe(refusal.meaning);
  });

  it('attributes an unenumerated code inside a band to its program, and no further', () => {
    expect(CORE).toBeDefined();
    if (CORE === undefined) return;
    const unused = CORE.base + CORE.span - 1;
    expect(PROTOCOL_REFUSALS.some((entry) => entry.code === unused)).toBe(false);
    const attribution = attributeCustomCode(unused);
    expect(attribution.disposition).toBe('banded');
    if (attribution.disposition !== 'banded') return;
    expect(attribution.band.label).toBe('core');
    expect(describeAttribution(attribution)).toContain('no refusal is declared at this code');
  });

  it('separates an unallocated band from a foreign one', () => {
    const highest = REFUSAL_BANDS.reduce((held, band) => Math.max(held, band.base + band.span), 0);
    expect(attributeCustomCode(highest).disposition).toBe('unbanded');
  });

  it('never confuses a test caller with a deployed program', () => {
    expect(TEST_CALLER).toBeDefined();
    if (TEST_CALLER === undefined) return;
    const attribution = attributeCustomCode(TEST_CALLER.base);
    expect(attribution.disposition === 'named' || attribution.disposition === 'banded').toBe(true);
    const band = attribution.disposition === 'named' || attribution.disposition === 'banded' ? attribution.band : null;
    expect(band?.tier).toBe('test-caller');
  });

  it('gives every band a disjoint claim on the code space', () => {
    for (const band of REFUSAL_BANDS) {
      expect(bandForCode(band.base)?.label).toBe(band.label);
      expect(bandForCode(band.base + band.span - 1)?.label).toBe(band.label);
      expect(bandForCode(band.base - 1)?.label).not.toBe(band.label);
    }
  });
});

describe('reading a refusal off the logs', () => {
  const CHILD = 'ChiLdProgram11111111111111111111111111111111';
  const PARENT = 'ParentProgram1111111111111111111111111111111';

  it('takes the LAST code, because the outermost frame has the last word', () => {
    expect(CORE && CLAIMS).toBeTruthy();
    if (CORE === undefined || CLAIMS === undefined) return;
    const child = CLAIMS.base + 0x11;
    const parent = CORE.base + 0x05;
    const reported = readReportedRefusal(
      [
        `Program ${PARENT} invoke [1]`,
        `Program ${CHILD} invoke [2]`,
        `Program ${CHILD} failed: custom program error: 0x${child.toString(16)}`,
        `Program ${PARENT} failed: custom program error: 0x${parent.toString(16)}`,
      ],
      null,
    );
    expect(reported?.code).toBe(parent);
    expect(reported?.program).toBe(PARENT);
  });

  it('credits a propagated code to the FIRST frame that reported it', () => {
    expect(CLAIMS).toBeDefined();
    if (CLAIMS === undefined) return;
    const code = CLAIMS.base + 0x08;
    const hex = code.toString(16);
    const reported = readReportedRefusal(
      [
        `Program ${PARENT} invoke [1]`,
        `Program ${CHILD} invoke [2]`,
        `Program ${CHILD} failed: custom program error: 0x${hex}`,
        `Program ${PARENT} failed: custom program error: 0x${hex}`,
      ],
      null,
    );
    // The parent re-reported the child's code while unwinding; only the child
    // originated it. Crediting the parent is the mirror this rule closes.
    expect(reported?.program).toBe(CHILD);
    expect(reported?.source).toBe('log-frame');
  });

  it('takes an unattributed code and says plainly that no program was named', () => {
    const reported = readReportedRefusal(['custom program error: 0x3005'], null);
    expect(reported?.code).toBe(0x3005);
    expect(reported?.program).toBeNull();
    expect(reported?.source).toBe('log-line');
  });

  it('falls back to the structured error when the logs carry nothing', () => {
    const reported = readReportedRefusal([], { InstructionError: [0, { Custom: 12294 }] });
    expect(reported?.code).toBe(12294);
    expect(reported?.source).toBe('transaction-error');
  });

  it('returns nothing when there is no custom code anywhere', () => {
    expect(readReportedRefusal(['Program X invoke [1]'], null)).toBeNull();
    expect(readReportedRefusal([], { InstructionError: [0, 'PrivilegeEscalation'] })).toBeNull();
  });

  it('reads the invoked frames as the chain reports them, with depth', () => {
    const frames = invokedFrames([
      `Program ${PARENT} invoke [1]`,
      `Program ${CHILD} invoke [2]`,
      `Program ${CHILD} success`,
      `Program ${PARENT} success`,
    ]);
    expect(frames.map((frame) => [frame.program, frame.depth])).toEqual([
      [PARENT, 1],
      [CHILD, 2],
    ]);
  });
});

describe('non-custom runtime errors', () => {
  it('reports the runtime’s own words rather than translating them', () => {
    expect(runtimeErrorLabel({ InstructionError: [2, 'PrivilegeEscalation'] })).toBe(
      'InstructionError #2: PrivilegeEscalation',
    );
    expect(runtimeErrorLabel('AlreadyProcessed')).toBe('AlreadyProcessed');
    expect(runtimeErrorLabel(null)).toBeNull();
  });

  it('finds a Custom code nested at any depth', () => {
    expect(customCodeFromError({ InstructionError: [1, { Custom: 20481 }] })).toBe(20481);
    expect(customCodeFromError({ a: { b: { c: { Custom: 7 } } } })).toBe(7);
    expect(customCodeFromError({ InstructionError: [1, 'MissingAccount'] })).toBeNull();
  });
});
