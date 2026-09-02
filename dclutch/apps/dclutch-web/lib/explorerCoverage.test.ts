import { describe, expect, it } from 'vitest';

import * as coverage from '../scripts/explorer-coverage.mjs';
import { decodeAgainstSpec, headerEndOf, magicText, renderedRecords, specForMagic } from './explorer/accountRecords';
import { instructionRenderers } from './explorer/instructions';

type CoverageRow = Readonly<{
  magic: string;
  state: 'rendered' | 'exempt' | 'unrendered';
  reason: string | null;
}>;

const report = coverage.coverage as () => Readonly<{
  records: ReadonlyArray<CoverageRow>;
  instructions: ReadonlyArray<CoverageRow>;
  magiclessLayouts: ReadonlyArray<CoverageRow>;
}>;
const surveyRecordMagics = coverage.surveyRecordMagics as () => ReadonlyArray<
  Readonly<{ magic: string; constants: ReadonlyArray<Readonly<{ module: string; constant: string }>> }>
>;
const surveyInstructionMagics = coverage.surveyInstructionMagics as () => ReadonlyArray<
  Readonly<{ magic: string; routes: ReadonlyArray<Readonly<{ routeId: string }>> }>
>;

/**
 * The explorer's done-criterion.
 *
 * `lib/generated/` is where every record magic, instruction magic and byte
 * offset the protocol has arrives in the browser — emitted from a Lean schema,
 * a Rust contract, or the route census, each behind its own `abi:*:verify`
 * gate. The explorer's job is to render what arrives.
 *
 * These tests hold that to a ratchet, in the same shape as `abiCoverage.test.ts`
 * holds the hand-mirror inventory. A magic that a generated module declares and
 * the explorer does not render fails the build. There are exactly two ways to
 * clear a failure: render the record, or record it in
 * `scripts/explorer-coverage.exempt.json` with a reason someone has to write
 * down. There is no third way, and in particular there is no way for a new
 * record to land and go quietly unrendered — which is the failure this whole
 * gate exists to prevent, because an explorer that silently skips a record does
 * not look broken, it looks like the record does not exist.
 *
 * Run `npm run explorer:coverage` to read the table.
 */
describe('explorer coverage', () => {
  it('renders every record magic the generated modules declare', () => {
    const missing = report()
      .records.filter((row) => row.state === 'unrendered')
      .map((row) => row.magic);
    expect(missing, 'render it in lib/explorer/accountRecords.ts, or exempt it with a reason').toEqual([]);
  });

  it('renders every instruction magic the route census enumerates', () => {
    const missing = report()
      .instructions.filter((row) => row.state === 'unrendered')
      .map((row) => row.magic);
    expect(missing, 'name its census route in lib/explorer/instructions.ts, or exempt it with a reason').toEqual([]);
  });

  it('catches a generated module that emits a layout without its magic', () => {
    // The inverse failure, and the one the join above is structurally blind to:
    // a magic that was never declared cannot appear in a survey of declared
    // magics. A module that emits a `..._MAGIC_OFFSET` and no magic value hands
    // the browser everything it needs to read a record and no way to know it is
    // looking at one — which invites a decode the emission cannot justify.
    const missing = report()
      .magiclessLayouts.filter((row) => row.state === 'unrendered')
      .map((row) => row.magic);
    expect(missing, 'emit the magic value alongside its offset, or exempt the module with a reason').toEqual([]);
  });

  it('carries a written reason for every exemption', () => {
    for (const row of [...report().records, ...report().instructions, ...report().magiclessLayouts]) {
      if (row.state !== 'exempt') continue;
      expect(row.reason, `${row.magic} is exempt with no reason`).toBeTruthy();
      expect((row.reason ?? '').length, `${row.magic}'s exemption reason is too short to be one`).toBeGreaterThan(24);
    }
  });

  it('exempts nothing it already renders', () => {
    const rendered = new Set(
      [...report().records, ...report().instructions]
        .filter((row) => row.state === 'rendered')
        .map((row) => row.magic),
    );
    for (const row of [...report().records, ...report().instructions]) {
      if (row.state === 'exempt') expect(rendered.has(row.magic)).toBe(false);
    }
  });
});

describe('the render map itself', () => {
  it('names no magic the generated modules do not declare', () => {
    const declared = new Set(surveyRecordMagics().map((entry) => entry.magic));
    for (const spec of renderedRecords()) {
      expect(declared.has(magicText(spec.magic)), `${magicText(spec.magic)} is rendered but not emitted`).toBe(true);
    }
  });

  it('names no census route that does not exist', () => {
    const known = new Set(surveyInstructionMagics().flatMap((entry) => entry.routes.map((route) => route.routeId)));
    for (const renderer of instructionRenderers()) {
      expect(known.has(renderer.routeId), `${renderer.routeId} is not a magic-selected census route`).toBe(true);
    }
  });

  it('resolves every body magic an instruction renderer names', () => {
    for (const renderer of instructionRenderers()) {
      if (renderer.bodyMagic === undefined) continue;
      expect(specForMagic(renderer.bodyMagic), `${renderer.routeId} names an unrendered body`).not.toBeNull();
    }
  });

  it('holds each record to exactly one spec', () => {
    const seen = new Set<string>();
    for (const spec of renderedRecords()) {
      const magic = magicText(spec.magic);
      expect(seen.has(magic), `${magic} is rendered twice`).toBe(false);
      seen.add(magic);
    }
  });

  it('states a reason whenever it renders no fields', () => {
    for (const spec of renderedRecords()) {
      if (spec.fields.length > 0) continue;
      expect(spec.note, `${magicText(spec.magic)} renders no fields and says nothing about why`).toBeTruthy();
    }
  });

  it('keeps every declared field inside its record', () => {
    for (const spec of renderedRecords()) {
      const end = headerEndOf(spec.width);
      for (const declared of spec.fields) {
        expect(declared.offset, `${magicText(spec.magic)}.${declared.label} starts past its header`).toBeLessThan(end);
      }
      // A class tail is bounded by ITS OWN class header, not the common prefix,
      // and it must start at or after the prefix -- a class field placed inside
      // the prefix would be two owners for one byte range.
      if (spec.width.kind !== 'action-classes') continue;
      for (const actionClass of spec.width.classes) {
        for (const declared of actionClass.fields) {
          expect(declared.offset, `${magicText(spec.magic)}/${actionClass.name}.${declared.label} starts before the common prefix ends`).toBeGreaterThanOrEqual(spec.width.commonPrefixBytes);
          expect(declared.offset, `${magicText(spec.magic)}/${actionClass.name}.${declared.label} starts past its class header`).toBeLessThan(actionClass.headerBytes);
        }
      }
    }
  });

  it('decodes a zeroed account of the right width without throwing', () => {
    for (const spec of renderedRecords()) {
      const width = headerEndOf(spec.width);
      const bytes = new Uint8Array(width);
      const magic = magicText(spec.magic);
      for (let index = 0; index < magic.length; index += 1) bytes[index] = magic.charCodeAt(index);
      const decoded = decodeAgainstSpec(spec, bytes);
      expect(decoded.magic).toBe(magic);
      // Every field either decodes or refuses with a stated reason; nothing
      // silently produces a wrong-looking value.
      for (const held of decoded.fields) {
        if (held.value.form === 'refused') expect(held.value.reason.length).toBeGreaterThan(0);
      }
    }
  });
});
