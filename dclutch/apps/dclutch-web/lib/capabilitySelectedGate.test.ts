import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import vector from '@dclutch/sdk/fixtures/state-machines.devnet.json';
import { ROUTE_SELECTED_GATES_V1 } from '@dclutch/sdk/generated/marketPhaseAdmissionV1';
import { STATE_MACHINE_RECORDS_V1 } from '@dclutch/sdk/generated/stateMachinesV1';
import {
  decodeDirectRootStateV1,
  machineObservationV1,
  type MachineObservationV1,
} from '@dclutch/sdk/stateMachines';
import {
  CAPABILITY_ACTIONS_V1,
  HOT_FAMILY_CLASSIFIERS_V1,
  capabilityActSelectedGatesV1,
  capabilitySelectedGateCoverageV1,
  capabilitySelectedGateSentenceV1,
  evaluateCapabilityV1,
  selectedTextV1,
  type CapabilityMarketSnapshotV1,
} from '@dclutch/sdk/capabilityModel';
import { BROWSER_CAPABILITY_STANDINGS_V1 } from './capabilitySurface';

/**
 * A gate behind a classifier, answered for the one family that reaches it.
 *
 * WHAT WAS WRONG, twice, in opposite directions. An unbounded necessary
 * descent through `trading/hot_v3::process_hot_execution_v3` reached the
 * Direct root's `Open` set and published it as a condition of the ROUTE --
 * which would have told four General and Dealer acts they need a root state
 * nothing in their execution reads. The census fixed that by moving both such
 * sets into `ROUTE_SELECTED_GATES_V1` with the classifier named, and the
 * result was the other error: the route then read as carrying no machine gate
 * at all, so the one act the Direct root really does bind reported READY TO
 * PREFLIGHT with that root unread.
 *
 * Both are the same missing coordinate. The route says which program arm runs;
 * the FAMILY says which prelude claims the request, and only the family can
 * decide whether a gate behind a decline is this act's gate. The families are
 * derived in `capabilityRouteDerivation.test.ts` from each builder's compiled
 * bytes; this file checks the other end -- that the classifier the census
 * names really is the family this model attributes it to, and that the arm
 * answers for that family and for nobody else.
 *
 * THE OBSERVATIONS ARE COHORT-15'S OWN. The Direct root below is the
 * activation root `FUJ9pNuk...`, its 24-byte lifecycle tail as read at slot
 * 492,837,406 -- not a constructed vector. Every assertion is the AGREEMENT
 * between what those bytes decode to and what the census's set says, so a root
 * that advances changes which branch runs and not whether this passes.
 */

const REPO = new URL('../../../', import.meta.url);

const hex = (value: string): Uint8Array =>
  Uint8Array.from((value.match(/../g) ?? []).map((byte) => Number.parseInt(byte, 16)));

/** The cohort-15 Direct root's tail, exactly as the devnet vector captured it. */
const COHORT_15_DIRECT_ROOT = 'FUJ9pNukWRb5658ysiDP5gz9gF3Hx8c4cU2mUrvELAWG';

function cohort15DirectRootTail(): Uint8Array {
  const found = vector.records.find(
    (record) => record.machine === 'direct-root' && record.address === COHORT_15_DIRECT_ROOT,
  );
  if (found === undefined) throw new Error('the devnet vector holds no cohort-15 Direct root');
  return hex(found.recordHex);
}

const standing = (id: string) => {
  const found = BROWSER_CAPABILITY_STANDINGS_V1.find((one) => one.action.id === id);
  if (found === undefined) throw new Error(`no standing for ${id}`);
  return found;
};

const observed: CapabilityMarketSnapshotV1 = {
  market: { address: '3QytL1bBMtCvRoXWR5h7MgutRBZqtv7emUVubEo5a4T2', phase: 'Open', readiness: 'Consumed' },
};

/** The one selected gate a Direct act reaches, looked up rather than typed. */
const directGate = () => {
  const classifier = HOT_FAMILY_CLASSIFIERS_V1.find((entry) => entry.family === 'Direct');
  if (classifier === undefined) throw new Error('no Direct classifier is bound');
  const gate = ROUTE_SELECTED_GATES_V1.find((entry) => entry.selectedBy === classifier.classifier);
  if (gate === undefined) throw new Error(`the census publishes no gate behind ${classifier.classifier}`);
  return gate;
};

/** An observation of one machine in one state, without going near the chain. */
const inState = (machine: string, state: string): MachineObservationV1 =>
  Object.freeze({ machine: machine as MachineObservationV1['machine'], present: true, state, refusal: null });

/**
 * A state the gate's own set does NOT admit, taken from the decoder's table.
 *
 * Not a literal. `Retiring` is what it resolves to today, and writing that
 * would make the hostile a test of one word: a set widened to admit every
 * state would still pass while refusing nothing.
 */
function stateOutsideV1(machine: string, states: ReadonlyArray<string>): string {
  const record = STATE_MACHINE_RECORDS_V1.find((entry) => entry.machine === machine);
  if (record === undefined) throw new Error(`${machine} has no generated record`);
  const outside = record.states.map((entry) => entry.state).find((state) => !states.includes(state));
  if (outside === undefined) throw new Error(`${machine} admits every state it has; the gate refuses nothing`);
  return outside;
}

describe('every classifier the census names is bound to one family, at its own source', () => {
  const PROGRAM_SRC = new URL('programs/dclutch-trading-sbf/src/', REPO);

  /**
   * The file one census classifier names, resolved from its module path.
   *
   * The census writes `selected_by` as it walks the AST, so the name carries
   * every module segment between the crate root and the function. Reading one
   * fixed file instead was what this did while `hot_v3.rs` was one file, and
   * the trading split (`hot_v3/direct.rs`, `hot_v3/series_expiry.rs`) made
   * that reading find nothing -- silently, because a classifier bound to no
   * source is not distinguishable here from one whose file moved. Resolving
   * the path makes a module move red at the classifier that moved.
   */
  function sourceOf(classifier: string): string {
    const segments = classifier.split('::').slice(0, -1);
    expect(segments.length, `${classifier} names no module path`).toBeGreaterThan(0);
    const stem = segments.join('/');
    for (const candidate of [`${stem}.rs`, `${stem}/mod.rs`]) {
      const path = fileURLToPath(new URL(candidate, PROGRAM_SRC));
      if (existsSync(path)) return readFileSync(path, 'utf8');
    }
    throw new Error(`${classifier}: no ${stem}.rs or ${stem}/mod.rs under the trading program`);
  }

  /** The classifier's text from its signature to the decline it opens with. */
  function declineOf(classifier: string): string {
    const bare = classifier.slice(classifier.lastIndexOf(':') + 1);
    const source = sourceOf(classifier);
    const start = source.indexOf(`fn ${bare}`);
    expect(start, `${bare} is not a function in the module ${classifier} names`).toBeGreaterThan(-1);
    const decline = source.indexOf('return Ok(None)', start);
    expect(decline, `${bare} never declines; a selected gate behind it would be unconditional`)
      .toBeGreaterThan(start);
    return source.slice(start, decline);
  }

  it('reads a decline, not a refusal, at the head of every bound classifier', () => {
    // The whole attribution rests on this: a classifier that returned `Err`
    // for another family would make its gate a condition of the ROUTE, and
    // publishing it per family would then be the false claim, inverted.
    expect(HOT_FAMILY_CLASSIFIERS_V1.length).toBeGreaterThan(0);
    for (const entry of HOT_FAMILY_CLASSIFIERS_V1) {
      expect(declineOf(entry.classifier).length).toBeGreaterThan(0);
    }
  });

  it('finds each classifier comparing its own family’s discriminant, and no other’s', () => {
    for (const entry of HOT_FAMILY_CLASSIFIERS_V1) {
      const decline = declineOf(entry.classifier);
      expect(decline, `${entry.classifier} does not compare ${entry.discriminant}`)
        .toContain(entry.discriminant);
      for (const other of HOT_FAMILY_CLASSIFIERS_V1) {
        if (other.family === entry.family) continue;
        expect(decline, `${entry.classifier} also names ${other.family}’s discriminant`)
          .not.toContain(other.discriminant);
      }
    }
  });

  it('finds each discriminant defined in the crate that owns its family’s wire', () => {
    // The second half of the attribution. Without it, a binding could name a
    // real discriminant and the wrong family: the crate is what ties
    // `DIRECT_SUCCESSOR_KIND_ID_V3` to the Direct wire rather than to a name
    // that merely reads like one.
    for (const entry of HOT_FAMILY_CLASSIFIERS_V1) {
      const name = entry.discriminant.split('::')[0]!;
      const source = fileURLToPath(new URL(`${entry.crate}/src`, REPO));
      const files = readdirSync(source, { recursive: true, encoding: 'utf8' })
        .filter((file) => file.endsWith('.rs'));
      const defined = files.some((file) => new RegExp(`\\bpub (const|enum|struct|type) ${name}\\b`)
        .test(readFileSync(`${source}/${file}`, 'utf8')));
      expect(defined, `${entry.crate} defines no ${name}`).toBe(true);
    }
  });

  it('leaves no selected gate the model cannot attribute to a family', () => {
    // A third selection the census starts publishing is red here until
    // somebody decides whose it is -- which is the only safe default: a gate
    // attributed to nobody is answered for nobody, and one attributed by
    // guess would refuse an act that never reaches it.
    for (const gate of ROUTE_SELECTED_GATES_V1) {
      expect(HOT_FAMILY_CLASSIFIERS_V1.map((entry) => entry.classifier), `${gate.selectedBy} is unbound`)
        .toContain(gate.selectedBy);
    }
    expect(capabilitySelectedGateCoverageV1(CAPABILITY_ACTIONS_V1).unclassified).toEqual([]);
  });
});

describe('the selected gate is answered for the family that takes the selection', () => {
  it('answers direct.inline from cohort-15’s own Direct root', () => {
    const decode = decodeDirectRootStateV1(cohort15DirectRootTail());
    expect(decode.status, decode.status === 'refused' ? decode.reason : '').toBe('decoded');
    if (decode.status !== 'decoded') return;

    const gate = directGate();
    const verdict = evaluateCapabilityV1(standing('direct.inline'), observed, [machineObservationV1(decode)]);
    expect(verdict.phaseGate.selectedGates).toHaveLength(1);
    const answered = verdict.phaseGate.selectedGates[0]!;
    expect(answered.machine).toBe(gate.machine);
    expect(answered.family).toBe('Direct');
    expect(answered.selectedBy).toBe(gate.selectedBy);
    expect(answered.observed).toBe(decode.state);
    // The agreement, never a state literal: the verdict is `admitted` exactly
    // when the root's decoded state is in the census's published set.
    expect(answered.verdict).toBe(gate.states.includes(decode.state) ? 'admitted' : 'excluded');
    expect(verdict.status).toBe(gate.states.includes(decode.state) ? 'ready-to-preflight' : 'wrong-phase');
    // The card says which classifier asked and on whose behalf.
    expect(selectedTextV1(verdict.phaseGate).join('; ')).toContain(gate.selectedBy);
    expect(selectedTextV1(verdict.phaseGate).join('; ')).toContain('Direct');
  });

  it('refuses a Direct act by the machine’s name when the root is outside the set', () => {
    const gate = directGate();
    const outside = stateOutsideV1(gate.machine, gate.states);
    const verdict = evaluateCapabilityV1(
      standing('direct.inline'), observed, [inState(gate.machine, outside)],
    );
    expect(verdict.status).toBe('wrong-phase');
    expect(verdict.reason).toContain(gate.machine);
    expect(verdict.reason).toContain(outside);
    expect(verdict.reason).toContain(gate.selectedBy);
    expect(verdict.reason).toContain('before any account is read');
    expect(verdict.phaseGate.selectedGates[0]!.verdict).toBe('excluded');
  });

  it('says needs-chain, naming the family, when nothing read the root', () => {
    const gate = directGate();
    const verdict = evaluateCapabilityV1(standing('direct.inline'), observed, []);
    expect(verdict.status).toBe('needs-chain');
    expect(verdict.phaseGate.verdict).toBe('other-machine');
    expect(verdict.phaseGate.unobservableMachines).toEqual([gate.machine]);
    expect(verdict.reason).toContain('Direct family');
    expect(verdict.reason).toContain(gate.machine);
  });
});

describe('the four other acts on the same route are asked nothing about it', () => {
  const gate = directGate();

  it.each(['general.consider', 'general.settle', 'general.close', 'dealer.liquidity'])(
    '%s reaches the same route and no Direct-root gate',
    (id) => {
      // THE PREVIOUS WRONG READING, pinned so it cannot come back. When the
      // Direct root's set was published as a condition of
      // `trading/hot_v3::process_hot_execution_v3`, every one of these four
      // declared that route and would have been told it needed a root state
      // nothing in its execution reads.
      const act = standing(id);
      expect(act.action.routes).toContain(gate.route);
      expect(capabilityActSelectedGatesV1(act.action, [inState(gate.machine, gate.states[0]!)])).toEqual([]);
      const refusing = evaluateCapabilityV1(
        act, observed, [inState(gate.machine, stateOutsideV1(gate.machine, gate.states))],
      );
      expect(refusing.phaseGate.selectedGates).toEqual([]);
      expect(refusing.status).not.toBe('wrong-phase');
      expect(refusing.reason).not.toContain(gate.machine);
      expect(refusing.phaseGate.unobservableMachines).toEqual([]);
    },
  );

  it('answers the Direct act on the very same observation, so silence proves something', () => {
    // The positive control the four cases above need: an arm that answered
    // nobody would pass every one of them.
    const gate = directGate();
    const refusing = [inState(gate.machine, stateOutsideV1(gate.machine, gate.states))];
    expect(evaluateCapabilityV1(standing('direct.inline'), observed, refusing).status).toBe('wrong-phase');
    expect(evaluateCapabilityV1(standing('general.consider'), observed, refusing).status)
      .toBe('ready-to-preflight');
  });
});

describe('the coverage sentence counts the selections rather than describing them', () => {
  it('names the acts that take one and the machine it costs them', () => {
    const coverage = capabilitySelectedGateCoverageV1(CAPABILITY_ACTIONS_V1);
    expect(coverage.gates).toBe(ROUTE_SELECTED_GATES_V1.length);
    expect(coverage.classified).toBe(coverage.gates);
    expect(coverage.acts).toEqual(['direct.inline']);
    expect(coverage.machines).toEqual([directGate().machine]);
    const sentence = capabilitySelectedGateSentenceV1(coverage);
    expect(sentence).toContain(String(coverage.gates));
    expect(sentence).toContain('direct.inline');
    expect(sentence).toContain(directGate().machine);
  });

  it('states that the Series ticket set is answered for nobody today', () => {
    // Not an absence: the census publishes `series-ticket: Prepared` behind the
    // Expire prelude, and no act on this board is a Series Expire, so the gate
    // is real and reaches no card. It will the moment one declares that family.
    const series = ROUTE_SELECTED_GATES_V1.find((gate) => gate.machine === 'series-ticket');
    expect(series, 'the census stopped publishing the Series selection').toBeDefined();
    expect(HOT_FAMILY_CLASSIFIERS_V1.some((entry) => entry.classifier === series!.selectedBy)).toBe(true);
    for (const act of CAPABILITY_ACTIONS_V1) {
      expect(capabilityActSelectedGatesV1(act, []).some((one) => one.machine === 'series-ticket')).toBe(false);
    }
  });
});
