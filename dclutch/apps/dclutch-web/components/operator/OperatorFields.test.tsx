import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { PublicKey } from '@solana/web3.js';

import {
  AtomsField,
  DerivedProvenance,
  DerivedValue,
  EnumField,
  EvidenceField,
  Hex64Field,
  KeypairHandoff,
  OperatorRefusal,
  PubkeyField,
} from './OperatorFields';
import { FieldRefusalV1 } from './fieldReadings';
import type { DenominationV1 } from '@dclutch/sdk/quantity';

const noop = () => undefined;
function key(seed: number): string {
  return new PublicKey(new Uint8Array(32).fill(seed)).toBase58();
}
const SIX_DECIMALS: DenominationV1 = { decimals: 6, unit: null, mint: key(9) };

describe('PubkeyField', () => {
  it('says nothing about an untouched field', () => {
    const html = renderToStaticMarkup(<PubkeyField label="Registry program" value="" onChange={noop} />);
    expect(html).toContain('Registry program');
    expect(html).not.toContain('operator-field-refusal');
    expect(html).not.toContain('operator-field-reading');
  });

  it('reports what it resolved, and names the account when the page knows it', () => {
    const html = renderToStaticMarkup(<PubkeyField
      label="Activation cache PDA"
      value={key(4)}
      onChange={noop}
      identify={() => 'the cache the step 02 plan derived'}
    />);
    expect(html).toContain('32 bytes');
    expect(html).toContain('the cache the step 02 plan derived');
    expect(html).toContain('aria-live="polite"');
  });

  it('puts the remedy and the detail in separate elements, remedy first', () => {
    const html = renderToStaticMarkup(<PubkeyField label="Payer" value="0OIl" onChange={noop} />);
    expect(html).toContain('role="alert"');
    expect(html).toContain('<strong>Paste the address as base58.</strong>');
    expect(html).toContain('omits 0, O, I and l');
    // The remedy element closes before the detail begins: two elements, in order.
    expect(html.indexOf('Paste the address as base58.')).toBeLessThan(html.indexOf('omits 0, O, I and l'));
  });

  it('never shows the library words the consoles show today', () => {
    const html = renderToStaticMarkup(<PubkeyField label="Core program" value="!!!not a key!!!" onChange={noop} />);
    expect(html).not.toContain('Invalid public key input');
    expect(html).not.toContain('Non-base58 character');
  });
});

describe('Hex64Field', () => {
  it('tells a digest field holding an address which of the two it has', () => {
    const html = renderToStaticMarkup(<Hex64Field label="Product record digest" value={key(6)} onChange={noop} />);
    expect(html).toContain('<strong>Paste the 64-character hex digest.</strong>');
    expect(html).toContain('names an account rather than a digest');
  });
});

describe('AtomsField', () => {
  it('keeps the exact atom count first and the humanized amount second', () => {
    const html = renderToStaticMarkup(<AtomsField
      label="Payout scale · atoms per unit"
      value="500000000"
      onChange={noop}
      denomination={SIX_DECIMALS}
    />);
    expect(html).toContain('500000000 atoms');
    expect(html).toContain('500 collateral at 6 decimals');
    // The exact integer survives as the thing typed, never replaced by the gloss.
    expect(html).toContain('value="500000000"');
  });

  it('refuses a decimal point at the field, not after a round trip', () => {
    const html = renderToStaticMarkup(<AtomsField label="Payout scale" value="1.5" onChange={noop} denomination={SIX_DECIMALS} />);
    expect(html).toContain('<strong>Enter this amount in whole atoms.</strong>');
    expect(html).toContain('does not divide');
  });
});

describe('EnumField', () => {
  it('offers every choice and glosses the selected one', () => {
    const html = renderToStaticMarkup(<EnumField
      label="Execution role"
      value="claims"
      onChange={noop}
      choices={['core', 'claims', 'trading']}
      describe={(choice) => choice === 'claims' ? 'the claims program' : null}
    />);
    expect(html).toContain('<option value="core">core</option>');
    expect(html).toContain('claims — the claims program');
  });
});

describe('EvidenceField', () => {
  const summarize = (parsed: unknown) => {
    const record = parsed as Record<string, unknown>;
    if (typeof record.format !== 'string') {
      throw new FieldRefusalV1(
        'Paste the clearing plan the operator program emitted.',
        'This parses as JSON but declares no format, so it is some other document.',
      );
    }
    return { identity: String(record.format), rows: [{ term: 'payer', detail: String(record.payer) }] };
  };

  it('renders a summary card the moment the JSON parses, instead of a blob', () => {
    const html = renderToStaticMarkup(<EvidenceField
      label="Clearing plan · JSON"
      value='{"format":"dclutch/general-successor-plan/v5","payer":"7Yk"}'
      onChange={noop}
      summarize={summarize}
    />);
    expect(html).toContain('operator-evidence-card');
    expect(html).toContain('reads as');
    expect(html).toContain('dclutch/general-successor-plan/v5');
    expect(html).toContain('<dt>payer</dt><dd>7Yk</dd>');
  });

  it('refuses a truncated paste without losing what the reader typed', () => {
    const html = renderToStaticMarkup(<EvidenceField label="Clearing plan" value='{"format":' onChange={noop} summarize={summarize} />);
    expect(html).toContain('Paste the whole file');
    expect(html).toContain('not complete JSON yet');
    expect(html).not.toContain('operator-evidence-card');
  });

  it("shows the console's own refusal for well-formed JSON of the wrong shape", () => {
    const html = renderToStaticMarkup(<EvidenceField label="Clearing plan" value='{"other":1}' onChange={noop} summarize={summarize} />);
    expect(html).toContain('<strong>Paste the clearing plan the operator program emitted.</strong>');
    expect(html).toContain('declares no format');
  });
});

describe('DerivedProvenance', () => {
  it('says a value was filled from the read, and that it stays editable', () => {
    const html = renderToStaticMarkup(<DerivedProvenance
      derived={key(2)}
      value={key(2)}
      source="the deployment this browser is pointed at"
    absent="Pick a cluster to fill this." />);
    expect(html).toContain('<strong>Filled from the deployment this browser is pointed at.</strong>');
    expect(html).toContain('You can paste a different value');
  });

  it('marks an override as an override, and prints what the read said', () => {
    const html = renderToStaticMarkup(<DerivedProvenance
      derived={key(2)}
      value={key(3)}
      source="the deployment this browser is pointed at"
      absent="Pick a cluster to fill this." />);
    expect(html).toContain('Manually set.');
    expect(html).toContain('reads');
    expect(html).not.toContain('Filled from');
  });

  it('names the way to get the value when there is nothing to derive from', () => {
    const html = renderToStaticMarkup(<DerivedProvenance
      derived={null}
      value=""
      source="the deployment"
      absent="Pick a cluster in the header to fill this, or paste an address." />);
    expect(html).toContain('Pick a cluster in the header to fill this, or paste an address.');
  });
});

describe('DerivedValue', () => {
  it('shows the computed value and how it was computed', () => {
    const html = renderToStaticMarkup(<DerivedValue
      label="Product raw account"
      value={key(8)}
      derivation="Derived from the Registry program, the pinned Product record schema, and the Product record digest above." />);
    expect(html).toContain(key(8));
    expect(html).toContain('Derived from the Registry program');
  });

  it('says what it is waiting for rather than showing a wrong address', () => {
    const html = renderToStaticMarkup(<DerivedValue label="Product raw account" value={null} derivation="Derived from the digest above." />);
    expect(html).toContain('waiting on the fields above');
  });
});

describe('KeypairHandoff', () => {
  it('names the env var and the command instead of asking for a key path', () => {
    const html = renderToStaticMarkup(<KeypairHandoff
      what="the activation packet"
      envVar="DCLUTCH_AUTHORITY_KEYPAIR"
      invocation="dclutch-operator sign --packet activation.bin" />);
    expect(html).toContain('a key never enters this page');
    expect(html).toContain('DCLUTCH_AUTHORITY_KEYPAIR');
    expect(html).toContain('dclutch-operator sign --packet activation.bin');
    expect(html).toContain('This browser reads no files and holds no keys.');
    // The rule is that no control collects the key. There is no input here.
    expect(html).not.toContain('<input');
    expect(html).not.toContain('type="password"');
  });
});

describe('OperatorRefusal', () => {
  it('uses the shared alert primitive while keeping remedy before detail', () => {
    const html = renderToStaticMarkup(<OperatorRefusal remedy="Check the Market address." detail="Refused: Market is absent." />);
    expect(html).toContain('data-slot="alert"');
    expect(html).toContain('data-slot="alert-title"');
    expect(html).toContain('data-slot="alert-description"');
    expect(html.indexOf('Check the Market address.')).toBeLessThan(html.indexOf('Refused: Market is absent.'));
  });
});
