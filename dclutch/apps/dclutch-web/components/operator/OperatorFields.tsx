'use client';

import { type ReactNode } from 'react';

import {
  compactAddressV1,
  readAtomsV1,
  readEndpointV1,
  readEnumV1,
  readEvidenceV1,
  readHex64V1,
  readPubkeyV1,
  readU64V1,
  type EvidenceRowV1,
  type FieldReadingV1,
} from './fieldReadings';
import type { DenominationV1 } from '@/lib/quantity';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';

/**
 * The typed fields the operator consoles ask their questions with.
 *
 * Every one renders the same four parts in the same order, because they are
 * the four questions a reader has about a box they are being asked to fill:
 *
 *   1. the LABEL -- what is this?
 *   2. the CONTROL -- the box itself.
 *   3. the PROVENANCE line -- where does the value come from? (`.feed-forward`,
 *      the class `/release` already uses for exactly this, generalised here)
 *   4. the READING -- what did the field make of what I typed?
 *      (`.artifact-fact`'s job, which today exists on exactly one field type)
 *
 * Parts 1-3 exist today in scattered places. Part 4 exists on one field type
 * out of 106. Nothing here is invented; it is the best behaviour already in
 * the consoles, made available to all of them.
 *
 * The DOM is deliberately the shape the stylesheet already expects -- a
 * wrapping `<label>` with a bare `<span>` inside it -- so these fields inherit
 * `.direct-card` / `.trade-v3-card` styling wherever they are dropped, and the
 * new CSS is only the reading line.
 */

/** The shared frame: label, control, provenance, reading. */
function FieldFrame({
  label,
  provenance,
  reading,
  children,
}: Readonly<{
  label: string;
  provenance?: ReactNode;
  reading: FieldReadingV1;
  children: ReactNode;
}>) {
  return <label className="operator-field">
    <span>{label}</span>
    {children}
    {provenance === undefined ? null : <small className="feed-forward">{provenance}</small>}
    <FieldReading reading={reading} />
  </label>;
}

/**
 * What the field made of its own text.
 *
 * A refusal is two parts in this order and never the other one -- what to DO,
 * then what the field found -- each in its own element, the grammar
 * `components/trade/StepRefusal.tsx` established. Its rule about elements is
 * load-bearing here too: "a sentence wrapped half in a `<span>` for emphasis
 * stops being findable", and these sentences are pinned by `toContain` guards.
 */
export function FieldReading({ reading }: Readonly<{ reading: FieldReadingV1 }>) {
  if (reading.state === 'empty') return null;
  if (reading.state === 'refused') {
    return <span className="operator-field-refusal" role="alert">
      <strong>{reading.remedy}</strong>
      <span>{reading.detail}</span>
    </span>;
  }
  return <span className="operator-field-reading" aria-live="polite">{reading.identity}</span>;
}

/**
 * The DERIVE rule, as a component.
 *
 * WAVE.md's canonical-generation mandate (2026-08-31): "an expectation is
 * either DERIVED from chain state ... or GENERATED from the single author ...
 * Hand-carried pins are a defect class, not a style choice." A console field
 * whose value the page has already read is a hand-carried pin with a text
 * cursor in it, and there are 45 of them.
 *
 * So a derivable field pre-fills from the read and SAYS SO -- and stays
 * editable wherever overriding it is legitimate, which is most places: the
 * operator is frequently the person who knows the read is stale. What changes
 * is that overriding becomes a visible act rather than an indistinguishable
 * one. `/release` already writes these three sentences by hand at two of its
 * fields; this is that pattern with a name, so the other eight consoles can
 * have it too.
 */
export function DerivedProvenance({
  derived,
  value,
  source,
  absent,
  editable = true,
}: Readonly<{
  /** What the page's own read says this value is, or null when unread. */
  derived: string | null;
  /** What is in the box right now. */
  value: string;
  /** Where the derived value came from: "the activation cache derived in step 02". */
  source: string;
  /** What to say when there is nothing to derive from -- must name the way to get it. */
  absent: string;
  editable?: boolean;
}>) {
  if (derived === null) return <>{absent}</>;
  if (value === derived) {
    return <><strong>Filled from {source}.</strong>{editable ? ' You can paste a different value; this line will say so.' : ''}</>;
  }
  return <>Manually set. {source} reads {compactAddressV1(derived)}.</>;
}

/** A 32-byte account address. */
export function PubkeyField({
  label,
  value,
  onChange,
  provenance,
  identify,
  required = false,
}: Readonly<{
  label: string;
  value: string;
  onChange: (next: string) => void;
  provenance?: ReactNode;
  /** Names a resolved address the page already knows, e.g. "the fee payer". */
  identify?: (address: string) => string | null;
  required?: boolean;
}>) {
  return <FieldFrame label={label} provenance={provenance} reading={readPubkeyV1(value, identify)}>
    <input
      required={required}
      spellCheck={false}
      value={value}
      onChange={(event) => onChange(event.target.value.trim())}
    />
  </FieldFrame>;
}

/** A 32-byte digest, 64 hex characters. */
export function Hex64Field({
  label,
  value,
  onChange,
  provenance,
  required = false,
}: Readonly<{
  label: string;
  value: string;
  onChange: (next: string) => void;
  provenance?: ReactNode;
  required?: boolean;
}>) {
  return <FieldFrame label={label} provenance={provenance} reading={readHex64V1(value)}>
    <input
      required={required}
      spellCheck={false}
      value={value}
      onChange={(event) => onChange(event.target.value.trim())}
    />
  </FieldFrame>;
}

/**
 * An amount in atoms.
 *
 * Typed in atoms, glossed in the display denomination -- the inverse of the
 * trade panel, per FLOWFUL_IA_V1 §5.4's rule that operator surfaces keep raw
 * atoms as the primary rendering. The gloss comes from `formatQuantityV1`, so
 * the exact integer is never the derived number and never a float.
 */
export function AtomsField({
  label,
  value,
  onChange,
  denomination,
  provenance,
  required = false,
}: Readonly<{
  label: string;
  value: string;
  onChange: (next: string) => void;
  denomination: DenominationV1;
  provenance?: ReactNode;
  required?: boolean;
}>) {
  return <FieldFrame label={label} provenance={provenance} reading={readAtomsV1(value, denomination)}>
    <input
      required={required}
      inputMode="numeric"
      spellCheck={false}
      value={value}
      onChange={(event) => onChange(event.target.value.trim())}
    />
  </FieldFrame>;
}

/**
 * A plain unsigned integer: a count, a limit, a generation, an identity.
 *
 * The most common numeric field on these consoles and, before this, the least
 * served -- `inputMode="numeric"` on eight of them is a keyboard hint that
 * constrains nothing. `noun` is what the integer counts, and it appears in the
 * refusal so the message is about this field rather than about arithmetic.
 */
export function U64Field({
  label,
  value,
  onChange,
  noun,
  min,
  max,
  provenance,
  required = false,
}: Readonly<{
  label: string;
  value: string;
  onChange: (next: string) => void;
  /** What the integer counts: "compute units", "generation", "outcome index". */
  noun: string;
  min?: bigint;
  max?: bigint;
  provenance?: ReactNode;
  required?: boolean;
}>) {
  return <FieldFrame label={label} provenance={provenance} reading={readU64V1(value, { noun, min, max })}>
    <input
      required={required}
      inputMode="numeric"
      spellCheck={false}
      value={value}
      onChange={(event) => onChange(event.target.value.trim())}
    />
  </FieldFrame>;
}

/**
 * A JSON-RPC endpoint.
 *
 * Seven fields read one of these and every one of them answers a malformed
 * value with the URL constructor's `Invalid URL` -- the most-reached
 * unauthored refusal on the site. This field enforces the same scheme rule
 * `lib/rpc.ts` enforces, before a click rather than after a round trip.
 */
export function EndpointField({
  label,
  value,
  onChange,
  provenance,
  required = false,
}: Readonly<{
  label: string;
  value: string;
  onChange: (next: string) => void;
  provenance?: ReactNode;
  required?: boolean;
}>) {
  return <FieldFrame label={label} provenance={provenance} reading={readEndpointV1(value)}>
    <input
      required={required}
      spellCheck={false}
      value={value}
      onChange={(event) => onChange(event.target.value.trim())}
    />
  </FieldFrame>;
}

/** One of a known set, over the choices that actually exist. */
export function EnumField({
  label,
  value,
  onChange,
  choices,
  provenance,
  describe,
}: Readonly<{
  label: string;
  value: string;
  onChange: (next: string) => void;
  choices: ReadonlyArray<string>;
  provenance?: ReactNode;
  /** Optional per-choice gloss, so a role list is not five bare nouns. */
  describe?: (choice: string) => string | null;
}>) {
  const reading = readEnumV1(value, choices);
  const gloss = reading.state === 'resolved' ? describe?.(value) ?? null : null;
  return <FieldFrame
    label={label}
    provenance={provenance}
    reading={gloss === null ? reading : { state: 'resolved', identity: `${value} — ${gloss}` }}
  >
    <select value={value} onChange={(event) => onChange(event.target.value)}>
      {choices.map((choice) => <option key={choice} value={choice}>{choice}</option>)}
    </select>
  </FieldFrame>;
}

/**
 * A JSON artifact, rendered as what it is the moment it parses.
 *
 * `components/trade/TicketCard.tsx` already does this for one artifact type:
 * paste a ticket, see a ticket, with a drawer holding every signed field.
 * Every other JSON field on the site is a bare textarea whose first feedback
 * is a thrown error after you press the button. This is that card,
 * generalised -- the summary is the caller's, because only the console knows
 * what shape it asked for.
 */
export function EvidenceField({
  label,
  value,
  onChange,
  summarize,
  provenance,
  required = false,
  rows = 8,
}: Readonly<{
  label: string;
  value: string;
  onChange: (next: string) => void;
  summarize: (parsed: unknown) => Readonly<{ identity: string; rows: ReadonlyArray<EvidenceRowV1> }>;
  provenance?: ReactNode;
  required?: boolean;
  rows?: number;
}>) {
  const reading = readEvidenceV1(value, summarize);
  return <div className="operator-evidence">
    <label className="operator-field">
      <span>{label}</span>
      <textarea
        required={required}
        spellCheck={false}
        rows={rows}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
      {provenance === undefined ? null : <small className="feed-forward">{provenance}</small>}
    </label>
    {reading.state === 'empty' ? null : reading.state === 'refused'
      ? <span className="operator-field-refusal" role="alert">
        <strong>{reading.remedy}</strong>
        <span>{reading.detail}</span>
      </span>
      : <dl className="operator-evidence-card" aria-live="polite">
        <div><dt>reads as</dt><dd>{reading.identity}</dd></div>
        {reading.rows.map((row) => <div key={row.term}><dt>{row.term}</dt><dd>{row.detail}</dd></div>)}
      </dl>}
  </div>;
}

/**
 * A value the page computed, shown rather than asked for.
 *
 * The other half of the DERIVE rule. When a field is a PURE function of
 * fields already on the form -- `/product-v2`'s six record accounts are
 * exactly this, `deriveFinalizedRecordAddressesV1(registry, pinnedSchema,
 * digest)` -- asking for it is not neutral. It invites a paste that disagrees
 * with the arithmetic, and then the console checks an answer it could have
 * computed.
 *
 * So the value is derived and DISPLAYED, with the derivation named. It stays
 * copyable, and the raw value stays on screen: this is a demotion of the input
 * box, never of the fact.
 */
export function DerivedValue({
  label,
  value,
  derivation,
}: Readonly<{
  label: string;
  /** The computed value, or null when its inputs are not all readable yet. */
  value: string | null;
  /** How it was computed, in one concrete sentence naming the inputs. */
  derivation: string;
}>) {
  return <div className="operator-derived">
    <span>{label}</span>
    <code>{value ?? 'waiting on the fields above'}</code>
    <small className="feed-forward">{derivation}</small>
  </div>;
}

/**
 * The KEYPAIR rule.
 *
 * No key path is ever typed into a browser. A field asking for
 * `~/.config/solana/id.json` asks the reader to name secret material to a web
 * page, and it cannot even work -- the browser has no filesystem to read it
 * from, so the value is decorative right up until someone believes it is not.
 *
 * What replaces it is not a smaller input. It is the CLI invocation that does
 * the signing, with the environment variable named, so the act stays where the
 * key already lives.
 *
 * The audit found ZERO such fields across all nine consoles: signing goes
 * through Wallet Standard, and three consoles say so in their own footers.
 * This component exists to keep that record, not to repair it -- it is the
 * shape the next console reaches for when it needs an offline signature, so
 * that "ask for the path" never becomes the obvious move.
 */
export function KeypairHandoff({
  what,
  envVar,
  invocation,
}: Readonly<{
  /** What is being signed, as a noun phrase. */
  what: string;
  /** The environment variable naming the keypair. */
  envVar: string;
  /** The exact command, ready to copy. */
  invocation: string;
}>) {
  return <div className="operator-keypair">
    <p>Signing {what} needs a key, and a key never enters this page. Run it where the key already is:</p>
    <pre><code>{`export ${envVar}=/path/to/your/keypair.json\n${invocation}`}</code></pre>
    <p className="operator-field-reading">This browser reads no files and holds no keys. The command above is the whole handoff.</p>
  </div>;
}

/**
 * A refusal the form received, rendered at the field that owns it.
 *
 * OPERATOR_FORMS_V1 §6. The same two-part grammar as a field's own reading --
 * remedy first, detail whole -- but sourced from a routing table over the
 * refusal the SDK actually produced, rather than from reading the text in the
 * box. A console with sixteen fields and one `aria-live` line is telling its
 * reader that something is wrong somewhere behind them.
 */
export function OperatorRefusal({
  remedy,
  detail,
}: Readonly<{ remedy: string; detail: string }>) {
  return <Alert className="operator-field-refusal routed">
    <AlertTitle><strong>{remedy}</strong></AlertTitle>
    <AlertDescription><span>{detail}</span></AlertDescription>
  </Alert>;
}
