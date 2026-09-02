/**
 * Each route's admissible Market prestates, mirrored for clients.
 *
 * A client deciding whether an act can be attempted right now was, until this
 * module, deciding it from the market's EXISTENCE: `/workbench` reported READY
 * TO PREFLIGHT for every act on an already-open market, because
 * `evaluateCapabilityV1` had no phase to consult and nothing published one
 * (`2b0046fb`).
 *
 * The authority is the guard itself. `315f1931` gave Core's ten inline
 * `state.phase != Phase::Open` conditions names -- `MarketAdmissionV1`
 * constants beside the code that checks them -- and `7d24a851` taught the route
 * census to read those constants structurally out of the Rust AST and carry
 * them per route. So the chain of custody is the same one the refusal registry
 * uses, arrow for arrow:
 *
 *   the guard's own constant -> census inventory -> docs/reference/routes.md
 *   -> this module,
 *
 * with `tools/genref/generate.sh --check` byte-gating the middle arrow and
 * `--check` here gating the last one.
 *
 * WHAT A CELL COMPOSES. A route's cell is a conjunction of gates separated by
 * `; `, and each gate may be a disjunction of alternatives separated by
 * ` or ` -- the two branches of one guard written as a selection, of which
 * exactly one runs. The admissible set is therefore the INTERSECTION over
 * conjuncts of the UNION within each, computed over all fifteen
 * `(phase, readiness)` pairs. Merging the phase lists instead, which this
 * parser did while no route carried two gates, unions what should intersect.
 *
 * WHAT THIS TABLE IS NOT. It is a NECESSARY condition per route and never a
 * sufficient one. An act whose prestate is excluded cannot succeed, and that
 * is a refutation a client may publish. An act whose prestate is admitted has
 * every account, release, request and child-acknowledgement check still ahead
 * of it, and no consumer may read admission as readiness.
 *
 * A route absent from this table has NO PUBLISHED GATE, which is a different
 * claim from "admits every phase": most routes are authoring routes with no
 * Market phase to consult, and some have a guard still written inline. The
 * count of routes read is emitted alongside so a consumer can say how much of
 * the surface the table covers rather than implying it covers all of it.
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('../../../', import.meta.url);
const referenceSource = readFileSync(new URL('docs/reference/routes.md', root), 'utf8');
const outputUrl = new URL('../lib/generated/marketPhaseAdmissionV1.ts', import.meta.url);

const PHASES = ['Founding', 'Open', 'Terminal', 'Retiring', 'Retired'];
const READINESS = ['Prepaid', 'Ready', 'Consumed'];

// docs/reference/routes.md: one `## <program label>` section per program, each
// row `| \`route\` | kind | selector | phase | status | \`provenance\` |`.
// A row with the wrong cell count is a format move, not a route, and throws
// rather than being skipped: a parser that silently drops rows would report an
// empty table as a clean one, which is the failure this whole chain exists to
// remove.
const routes = [];
for (const line of referenceSource.split('\n')) {
  if (!line.startsWith('| `')) continue;
  const cells = line.slice(2, line.length - 2).split(' | ');
  if (cells.length !== 6) throw new Error(`routes.md row has ${cells.length} cells, not 6: ${line.slice(0, 120)}`);
  const route = cells[0].replace(/^`|`$/g, '');
  routes.push({ route, phase: cells[3].trim() });
}
if (routes.length < 100) throw new Error(`only ${routes.length} route rows parsed from routes.md; the format moved`);

// A cell is a CONJUNCTION of gates separated by `; `, and each gate may be a
// DISJUNCTION of alternatives separated by ` or ` -- the two branches of one
// guard written as a selection, of which exactly one runs. So the route's
// admissible prestates are the intersection over conjuncts of the union over
// each conjunct's alternatives, computed here over all fifteen pairs rather
// than approximated by merging phase lists. Merging was what this parser used
// to do, and it was invisible only because no route had ever carried two.
const pairKey = (phase, readiness) => `${phase}+${readiness}`;
const EVERY_PAIR = PHASES.flatMap((phase) => READINESS.map((readiness) => pairKey(phase, readiness)));

function declarationPairs(route, declaration) {
  const pairs = new Set();
  for (const term of declaration.replace(/^`|`$/g, '').split(', ')) {
    const [phase, readiness] = term.split('+');
    if (!PHASES.includes(phase)) throw new Error(`route ${route} names phase ${phase}, which is not a Core phase`);
    if (readiness === undefined) {
      // A guard that names no readiness admits every one, which is what
      // `MarketAdmissionV1::phases` means and what the page prints bare.
      for (const every of READINESS) pairs.add(pairKey(phase, every));
      continue;
    }
    if (!READINESS.includes(readiness)) {
      throw new Error(`route ${route} names readiness ${readiness}, which is not a Core readiness`);
    }
    pairs.add(pairKey(phase, readiness));
  }
  return pairs;
}

const gates = [];
for (const entry of routes) {
  if (entry.phase === 'no phase gate') continue;
  let admitted = new Set(EVERY_PAIR);
  for (const conjunct of entry.phase.split('; ')) {
    const united = new Set();
    for (const alternative of conjunct.split(' or ')) {
      for (const pair of declarationPairs(entry.route, alternative)) united.add(pair);
    }
    admitted = new Set([...admitted].filter((pair) => united.has(pair)));
  }
  if (admitted.size === 0) {
    // Contradictory gates on one route: either the census attributed a gate
    // to a route that cannot reach it, or the route is genuinely dead. Both
    // are findings, and publishing an empty set as a table row would hide
    // them behind a client that simply refuses everything.
    throw new Error(`route ${entry.route} admits no prestate at all: ${entry.phase}`);
  }
  // `prestates` stays empty for a set that admits every readiness in each of
  // its phases -- the phase-projection shape a phase-only guard has -- so the
  // exact column keeps meaning "this guard constrained readiness".
  const phases = PHASES.filter((phase) => READINESS.some((readiness) => admitted.has(pairKey(phase, readiness))));
  const exact = phases.some((phase) => !READINESS.every((readiness) => admitted.has(pairKey(phase, readiness))));
  const prestates = exact
    ? phases.flatMap((phase) => READINESS.filter((readiness) => admitted.has(pairKey(phase, readiness))).map((readiness) => [phase, readiness]))
    : [];
  gates.push({ route: entry.route, phases, prestates });
}
if (gates.length === 0) throw new Error('routes.md published no phase gate at all; the column is gone or empty');

// -------------------------------------------------------------------- output

const ts = (value) => JSON.stringify(value);
let generated = '// @generated from docs/reference/routes.md; do not edit.\n';
generated += '// Regenerate with: npm run abi:phase-admission\n\n';
generated += `/** Every phase a Core Market can be in. */\nexport type MarketPhaseV1 = ${PHASES.map(ts).join(' | ')};\n\n`;
generated += `/** Every Resolution Fund readiness a Core Market can be in. */\nexport type MarketReadinessV1 = ${READINESS.map(ts).join(' | ')};\n\n`;
generated += `/**
 * One route's admissible Market prestates, as its own guard declares them.
 *
 * \`prestates\` is exact and \`phases\` is its projection. A guard that names no
 * readiness leaves \`prestates\` empty and admits every readiness in \`phases\`.
 */
export interface RoutePhaseGateV1 {
  readonly route: string;
  readonly phases: ReadonlyArray<MarketPhaseV1>;
  readonly prestates: ReadonlyArray<readonly [MarketPhaseV1, MarketReadinessV1]>;
}\n\n`;
generated += `/** Routes enumerated by the census, gated or not. */\nexport const ROUTE_COUNT_V1 = ${routes.length} as const;\n\n`;
generated += 'export const ROUTE_PHASE_GATES_V1: ReadonlyArray<RoutePhaseGateV1> = [\n';
for (const gate of gates) {
  const prestates = gate.prestates.map(([phase, readiness]) => `[${ts(phase)}, ${ts(readiness)}]`).join(', ');
  generated += `  { route: ${ts(gate.route)}, phases: [${gate.phases.map(ts).join(', ')}], prestates: [${prestates}] },\n`;
}
generated += '];\n\n';
generated += `/** The gate for one route, or \`null\` when the census read none for it. */
export function routePhaseGateV1(route: string): RoutePhaseGateV1 | null {
  return ROUTE_PHASE_GATES_V1.find((gate) => gate.route === route) ?? null;
}\n`;

if (process.argv.includes('--check')) {
  const current = readFileSync(outputUrl, 'utf8');
  if (current !== generated) {
    console.error('market phase admission TypeScript mirror is stale');
    process.exit(1);
  }
} else {
  writeFileSync(fileURLToPath(outputUrl), generated);
}
