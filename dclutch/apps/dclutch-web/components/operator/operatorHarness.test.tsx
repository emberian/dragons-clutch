import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { PublicKey } from '@solana/web3.js';

import {
  AtomsField,
  DerivedValue,
  EndpointField,
  EnumField,
  EvidenceField,
  Hex64Field,
  KeypairHandoff,
  OperatorRefusal,
  PubkeyField,
  U64Field,
} from './OperatorFields';
import { FieldRefusalV1 } from './fieldReadings';
import type { DenominationV1 } from '@/lib/quantity';

/**
 * The narrow-width guard for the operator field vocabulary, in two parts.
 *
 * Part one is a CSS INVARIANT, checked here and needing no browser. Every
 * horizontal overflow this stylesheet could cause has exactly three possible
 * sources -- a grid track that cannot shrink below its content, a fixed width,
 * or an unbreakable mono string -- and all three are checkable as properties
 * of the rules themselves. `minmax(0, ...)` on every track is the one that
 * actually matters: `1fr` alone is `minmax(auto, 1fr)`, which refuses to
 * shrink below its content, and a 44-character base58 address in a mono font
 * is exactly the content that then pushes a 390px viewport sideways.
 *
 * Part two follows `components/trade/flowHarness.test.tsx`: it writes the real
 * components with real props into one page beside the real stylesheet, so a
 * browser can measure what a static assertion cannot. That file is only
 * written when FLOW_HARNESS_DIR (or OPERATOR_HARNESS_DIR) is set, so the suite
 * stays a suite.
 */

const CSS = readFileSync(join(import.meta.dirname, '..', '..', 'app', 'globals.css'), 'utf8');

/** The operator block, which is the only part this lane is responsible for. */
const OPERATOR_CSS = CSS.slice(CSS.indexOf('Operator typed fields -- components/operator/'));

describe('the operator field stylesheet cannot overflow a narrow viewport', () => {
  it('gives every grid track a zero minimum, so content can never set the floor', () => {
    // `grid-template-columns: repeat(2, 1fr)` is the bug: `1fr` means
    // `minmax(auto, 1fr)`, and `auto` is the widest unbreakable content --
    // a 44-character address. Every track here must say minmax(0, ...).
    const tracks = OPERATOR_CSS.match(/grid-template-columns:[^;]+;/g) ?? [];
    expect(tracks.length).toBeGreaterThan(0);
    for (const track of tracks) {
      expect(track, `${track} has a track that cannot shrink`).not.toMatch(/(?<!minmax\(0,\s)\b1fr\b/);
      expect(track).toContain('minmax(0');
    }
  });

  it('breaks every mono value that could be longer than the column', () => {
    // Addresses, digests and atom counts are unbreakable words to a browser.
    for (const rule of ['.operator-field-reading', '.operator-evidence-card dd', '.operator-derived code']) {
      const at = OPERATOR_CSS.indexOf(rule);
      expect(at, `${rule} missing`).toBeGreaterThan(-1);
      const body = OPERATOR_CSS.slice(at, OPERATOR_CSS.indexOf('}', at));
      expect(body, `${rule} must break long values`).toContain('overflow-wrap: anywhere');
    }
  });

  it('lets the one block that cannot wrap scroll inside itself instead', () => {
    // A shell invocation must not be re-wrapped -- it would stop being
    // copy-pasteable -- so it scrolls in its own box rather than the page.
    const at = OPERATOR_CSS.indexOf('.operator-keypair pre');
    expect(at).toBeGreaterThan(-1);
    expect(OPERATOR_CSS.slice(at, OPERATOR_CSS.indexOf('}', at))).toContain('overflow-x: auto');
  });

  it('collapses every multi-column grid before the viewport gets narrow', () => {
    expect(OPERATOR_CSS).toContain('@media (max-width: 760px)');
    const media = OPERATOR_CSS.slice(OPERATOR_CSS.indexOf('@media (max-width: 760px)'));
    expect(media).toContain('.operator-act-grid { grid-template-columns: minmax(0, 1fr); }');
  });

  it('sets no fixed width anywhere, so nothing can exceed 390px by construction', () => {
    const widths = OPERATOR_CSS.match(/[^-]width:\s*(\d+)px/g) ?? [];
    expect(widths).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// The browser subject.
// ---------------------------------------------------------------------------

const OUT_DIR_V1 = process.env.OPERATOR_HARNESS_DIR ?? process.env.FLOW_HARNESS_DIR ?? '';
const noop = () => undefined;
const key = (seed: number) => new PublicKey(new Uint8Array(32).fill(seed)).toBase58();
const SIX_DECIMALS: DenominationV1 = Object.freeze({ decimals: 6, unit: null, mint: key(9) });

describe('operator field layout harness', () => {
  it('writes every field in every state for a browser to measure', () => {
    // Both states of every type, because a refusal is taller and wider than a
    // reading and it is the refusal that carries the long sentences.
    const page = renderToStaticMarkup(<main className="product-shell direct-workspace">
      <section className="direct-card">
        <fieldset className="operator-act">
          <legend>Every field, resolved</legend>
          <div className="operator-act-grid">
            <PubkeyField label="Registry program" value={key(3)} onChange={noop} identify={() => 'the Registry of the deployment this browser is pointed at'} />
            <Hex64Field label="Product record digest · 32 hex bytes" value={'a1'.repeat(32)} onChange={noop} />
            <AtomsField label="Payout scale · atoms per unit" value="18446744073709551615" onChange={noop} denomination={SIX_DECIMALS} />
            <U64Field label="Activation compute-unit limit" value="1400000" onChange={noop} noun="compute units" max={1_400_000n} />
            <EndpointField label="Finalized RPC endpoint" value="http://127.0.0.1:20890" onChange={noop} />
            <EnumField label="Execution role" value="claims" onChange={noop} choices={['core', 'claims', 'trading', 'resolution', 'custody']} describe={() => 'the claims program'} />
          </div>
        </fieldset>
        <fieldset className="operator-act">
          <legend>Every field, refusing</legend>
          <div className="operator-act-grid">
            <PubkeyField label="Payer" value={`${key(3).slice(0, 7)}0${key(3).slice(8)}`} onChange={noop} />
            <Hex64Field label="Portfolio record digest" value={key(5)} onChange={noop} />
            <AtomsField label="Payout scale" value="1.5" onChange={noop} denomination={SIX_DECIMALS} />
            <U64Field label="Market generation" value="0" onChange={noop} noun="generation" min={1n} />
            <EndpointField label="Finalized RPC endpoint" value="127.0.0.1:20890" onChange={noop} />
          </div>
        </fieldset>
        <DerivedValue label="Product staging account" value={key(8)}
          derivation="Derived from the Registry program, the pinned Product record schema, and the Product record digest above." />
        <OperatorRefusal remedy="Check the address in Linked basis raw." detail="Refused: finalized raw record 4 must be canonical base58 text" />
        <EvidenceField label="Clearing plan · JSON" value='{"format":"dclutch/general-successor-plan/v5","payer":"9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin"}'
          onChange={noop} summarize={(parsed) => {
            const record = parsed as Record<string, unknown>;
            if (typeof record.format !== 'string') throw new FieldRefusalV1('Paste the clearing plan.', 'This declares no format.');
            return { identity: record.format, rows: [{ term: 'payer', detail: String(record.payer) }] };
          }} />
        <KeypairHandoff what="the activation packet" envVar="DCLUTCH_AUTHORITY_KEYPAIR"
          invocation="dclutch-operator sign --packet activation.bin --keypair $DCLUTCH_AUTHORITY_KEYPAIR" />
      </section>
    </main>);

    expect(page).toContain('operator-field-refusal');
    expect(page).toContain('operator-derived');

    if (OUT_DIR_V1 === '') return;
    mkdirSync(OUT_DIR_V1, { recursive: true });
    writeFileSync(join(OUT_DIR_V1, 'operator-fields.html'),
      `<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">`
      + `<style>${CSS}</style><body>${page}</body>`);
  });
});
