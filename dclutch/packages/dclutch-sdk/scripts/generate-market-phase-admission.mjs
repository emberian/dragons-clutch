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
//
// WHICH TABLE, though. This scanned every line in the file that began with a
// backticked cell, which was every route row for as long as the route tables
// were the only such table. On 2026-09-03 routes.md grew a second one --
// "Campaign records naming routes the code does not", three cells wide,
// emitted the moment a lane deleted a route a stale binding still names -- and
// this generator stopped being runnable at all: it threw on the first orphan
// row, correctly refusing to guess, with nothing wrong in the page it was
// reading. So the section is part of the format now. A `## <label>` heading
// with one lowercase word is a program; anything else ends the route tables,
// and the six-cell refusal applies only inside them, where a wrong width still
// means the format moved.
const PROGRAM_HEADING = /^## [a-z0-9-]+$/;
// The one non-program section this generator READS rather than skips. A gate
// behind a family's classifier is enforced on the route and is not a condition
// OF the route, so `routes.md` prints it apart from the phase column; dropping
// it here is what kept `evaluateCapabilityV1` unable to answer the only two
// machine gates any act on the board can reach.
const SELECTION_HEADING = '## Gates behind a selection';
const routes = [];
const selected = [];
let inProgramSection = false;
let inSelectionSection = false;
for (const line of referenceSource.split('\n')) {
  if (line.startsWith('## ')) {
    inProgramSection = PROGRAM_HEADING.test(line);
    inSelectionSection = line === SELECTION_HEADING;
    continue;
  }
  if (!line.startsWith('| `')) continue;
  const cells = line.slice(2, line.length - 2).split(' | ');
  if (inSelectionSection) {
    if (cells.length !== 4) throw new Error(`a selection row has ${cells.length} cells, not 4: ${line.slice(0, 120)}`);
    selected.push({
      route: cells[0].replace(/^`|`$/g, ''),
      selectedBy: cells[1].replace(/^`|`$/g, ''),
      declaration: cells[2].trim(),
      provenance: cells[3].replace(/^`|`$/g, ''),
    });
    continue;
  }
  if (!inProgramSection) continue;
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

// `market: Open+Consumed` -> the machine and the terms. A set that names no
// machine is a page written before machines were named, and it throws rather
// than defaulting to `market`: defaulting is how a Source set would be checked
// against a Market phase.
function declarationMachine(route, declaration) {
  const body = declaration.replace(/^`|`$/g, '');
  const split = body.indexOf(': ');
  if (split < 0) throw new Error(`route ${route} names a set with no state machine: ${body}`);
  return { machine: body.slice(0, split), terms: body.slice(split + 2) };
}

function declarationPairs(route, terms) {
  const pairs = new Set();
  for (const term of terms.split(', ')) {
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
// A route gated on a machine this table cannot express, and which machines.
// Reported rather than dropped: a route whose only gate is over the Source
// resolution state is NOT ungated, and a client told it was ungated would
// report an admission the chain refuses.
const otherMachines = [];
// Routes whose PROGRAM persists no lifecycle discriminant at all, as the
// census declares it. A different fact from an absent gate, and it has to
// travel: told "no gate was read", a client keeps waiting for an answer that
// no future naming will produce, and shows the same "not checked" text
// forever for a route where "nothing to check" is the truth.
const noStateMachine = [];
for (const entry of routes) {
  if (entry.phase === 'no state machine') {
    noStateMachine.push(entry.route);
    continue;
  }
  if (entry.phase === 'no phase gate') continue;
  let admitted = new Set(EVERY_PAIR);
  let sawMarket = false;
  const machines = [];
  // The same algebra the Market half runs, over each other machine's own state
  // names: union inside a conjunct, intersection across conjuncts on one
  // machine. These used to be DROPPED -- the table carried the machine's name
  // and not its set -- so a client that could decode a Direct root still had
  // nothing to compare the decoded phase against, and `needs-chain` was the
  // only answer available even with the account in hand.
  const otherAdmitted = new Map();
  for (const conjunct of entry.phase.split('; ')) {
    const united = new Set();
    let machine = null;
    for (const alternative of conjunct.split(' or ')) {
      const declaration = declarationMachine(entry.route, alternative);
      if (machine !== null && machine !== declaration.machine) {
        throw new Error(`route ${entry.route} unites sets over two machines: ${entry.phase}`);
      }
      machine = declaration.machine;
      if (machine === 'market') {
        for (const pair of declarationPairs(entry.route, declaration.terms)) united.add(pair);
        continue;
      }
      for (const state of declaration.terms.split(', ')) {
        const name = state.trim();
        if (!/^[A-Z][A-Za-z0-9]*$/.test(name)) {
          throw new Error(`route ${entry.route} names ${machine} state ${name}, which is not a state name`);
        }
        united.add(name);
      }
    }
    if (!machines.includes(machine)) machines.push(machine);
    if (machine !== 'market') {
      const standing = otherAdmitted.get(machine);
      otherAdmitted.set(machine, standing === undefined
        ? united
        : new Set([...standing].filter((state) => united.has(state))));
      continue;
    }
    sawMarket = true;
    admitted = new Set([...admitted].filter((pair) => united.has(pair)));
  }
  const other = machines.filter((one) => one !== 'market');
  if (other.length > 0) {
    const sets = other.map((machine) => {
      const states = [...otherAdmitted.get(machine)].sort();
      if (states.length === 0) {
        // The same finding the Market half throws on, one machine over: two
        // conjuncts on one machine that share no state means the census
        // attributed a gate to a route that cannot reach it, or the route is
        // dead. Publishing an empty set would hide it behind a client that
        // simply refuses everything.
        throw new Error(`route ${entry.route} admits no ${machine} state at all: ${entry.phase}`);
      }
      return { machine, states };
    });
    otherMachines.push({ route: entry.route, machines: other, gates: sets });
  }
  if (!sawMarket) continue;
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

// The selection rows, in the same shape the machine sets take, so a consumer
// answers one from a decoded observation exactly as it answers a necessary
// gate -- and never from the route alone.
const selectedGates = selected.map((entry) => {
  const declaration = declarationMachine(entry.route, entry.declaration);
  if (declaration.machine === 'market') {
    // A Market prestate behind a classifier would be a real finding and this
    // table has no field for the pair algebra the phase column runs. Refusing
    // is the honest end: a row published as a bare state list would be checked
    // against a phase name that is not one.
    throw new Error(`selection ${entry.selectedBy} names a market set, which this table cannot carry: ${entry.declaration}`);
  }
  const states = declaration.terms.split(', ').map((state) => state.trim());
  for (const state of states) {
    if (!/^[A-Z][A-Za-z0-9]*$/.test(state)) {
      throw new Error(`selection ${entry.selectedBy} names ${declaration.machine} state ${state}, which is not a state name`);
    }
  }
  return { route: entry.route, selectedBy: entry.selectedBy, machine: declaration.machine, states, provenance: entry.provenance };
});

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
}\n\n`;
generated += `/**
 * Routes whose admissibility is over a state machine this table cannot state.
 *
 * A Source resolution state, a Dealer root's lifecycle, a Series ticket: none
 * of them is the Core Market's phase, and a Market is \`Open\` for the whole
 * span in which its Source moves \`Primary\` to \`Resolved\`. So these routes are
 * NOT ungated, and a consumer that treated them as ungated would report an
 * admission the chain refuses. A consumer that cannot observe the named
 * machine must say \`needs-chain\` and not \`no-phase-gate\`.
 */
export interface RouteOtherMachineGateV1 {
  readonly route: string;
  readonly machines: ReadonlyArray<string>;
  /**
   * The states each of those machines admits, as the guard declares them.
   *
   * A reader holding a decoded observation of the machine answers the gate
   * from this set. Only a reader that has NO observation says \`needs-chain\`,
   * and it says which machine it is missing.
   */
  readonly gates: ReadonlyArray<RouteMachineStatesV1>;
}

/** One machine's admissible states on one route. */
export interface RouteMachineStatesV1 {
  readonly machine: string;
  readonly states: ReadonlyArray<string>;
}\n\n`;
generated += 'export const ROUTES_GATED_ON_ANOTHER_MACHINE_V1: ReadonlyArray<RouteOtherMachineGateV1> = [\n';
for (const entry of otherMachines) {
  const sets = entry.gates.map((set) => `{ machine: ${ts(set.machine)}, states: [${set.states.map(ts).join(', ')}] }`).join(', ');
  generated += `  { route: ${ts(entry.route)}, machines: [${entry.machines.map(ts).join(', ')}], gates: [${sets}] },\n`;
}
generated += '];\n\n';
generated += `/** The machines gating one route that this table cannot state, if any. */
export function routeOtherMachineGateV1(route: string): RouteOtherMachineGateV1 | null {
  return ROUTES_GATED_ON_ANOTHER_MACHINE_V1.find((entry) => entry.route === route) ?? null;
}

/** The states one machine admits on one route, or \`null\` when it gates neither. */
export function routeMachineStatesV1(route: string, machine: string): ReadonlyArray<string> | null {
  return routeOtherMachineGateV1(route)?.gates.find((set) => set.machine === machine)?.states ?? null;
}

/** Every machine any route in this table is gated on, once each. */
export function gatedMachinesV1(): ReadonlyArray<string> {
  const machines: string[] = [];
  for (const entry of ROUTES_GATED_ON_ANOTHER_MACHINE_V1) {
    for (const machine of entry.machines) if (!machines.includes(machine)) machines.push(machine);
  }
  return machines.sort();
}\n\n`;
generated += `/**
 * One gate that lies behind a CLASSIFIER'S DECLINE, not behind the route.
 *
 * One route can be the entry for several families -- Trading's
 * \`process_hot_execution_v3\` is the whole Hot surface -- and each family's
 * prelude returns a non-error for every request that is not its own before it
 * reads anything. What such a prelude then enforces is necessary to ITS family
 * and to no other, so it is not in \`ROUTE_PHASE_GATES_V1\` and not in
 * \`ROUTES_GATED_ON_ANOTHER_MACHINE_V1\`: written into either it would tell the
 * four other acts on the route that they need a ticket nobody in their
 * execution has, which is the false READY TO PREFLIGHT the phase gates were
 * built to remove, inverted.
 *
 * \`selectedBy\` names the function that declines. A consumer may answer one of
 * these ONLY for an execution it can show takes that selection -- which for a
 * capability act means its declared family, derived from the bytes its own
 * builder compiles.
 */
export interface RouteSelectedGateV1 {
  readonly route: string;
  /** The classifier whose decline this gate sits behind. */
  readonly selectedBy: string;
  readonly machine: string;
  readonly states: ReadonlyArray<string>;
  /** The Rust the census read the set out of. */
  readonly provenance: string;
}\n\n`;
generated += 'export const ROUTE_SELECTED_GATES_V1: ReadonlyArray<RouteSelectedGateV1> = [\n';
for (const entry of selectedGates) {
  generated += `  { route: ${ts(entry.route)}, selectedBy: ${ts(entry.selectedBy)}, machine: ${ts(entry.machine)}, states: [${entry.states.map(ts).join(', ')}], provenance: ${ts(entry.provenance)} },\n`;
}
generated += '];\n\n';
generated += `/** Every gate on one route that lies behind a classifier's decline. */
export function routeSelectedGatesV1(route: string): ReadonlyArray<RouteSelectedGateV1> {
  return ROUTE_SELECTED_GATES_V1.filter((entry) => entry.route === route);
}\n\n`;
generated += `/**
 * Routes whose program persists NO lifecycle discriminant for them to consult.
 *
 * Absent from \`ROUTE_PHASE_GATES_V1\` for a reason no further naming will
 * change: the Registry authenticates ownership, PDA derivation, account
 * vacancy and digest identity, and not one of those is a state byte. A client
 * told only "no gate was read" waits forever for an answer that does not
 * exist; a client told this can say so and move on. Still NOT an admission --
 * every account, release and request check is ahead of the act regardless.
 */
export const ROUTES_WITHOUT_A_STATE_MACHINE_V1: ReadonlyArray<string> = [\n`;
for (const route of noStateMachine) {
  generated += `  ${ts(route)},\n`;
}
generated += '];\n\n';
generated += `/** Whether this route's program has no lifecycle discriminant at all. */
export function routeHasNoStateMachineV1(route: string): boolean {
  return ROUTES_WITHOUT_A_STATE_MACHINE_V1.includes(route);
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
