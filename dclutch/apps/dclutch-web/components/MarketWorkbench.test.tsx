import { CAPABILITY_ACTIONS_V1 } from '@dclutch/sdk/capabilityModel';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import MarketWorkbench, { machineObservationTextV1, workbenchRefusalFieldV1 } from './MarketWorkbench';
import { STATE_MACHINE_RECORDS_V1 } from '@dclutch/sdk/generated/stateMachinesV1';
import { absentMachineObservationV1 } from '@dclutch/sdk/stateMachines';

describe('market lifecycle workbench', () => {
  it('says what each authoring act needs before it can begin, and never greys a control', () => {
    const html = renderToStaticMarkup(<MarketWorkbench />);
    expect(html).toContain('Lifecycle readiness');
    expect(html).toContain('Enter a market address to see its current stage');
    expect(html).toContain('available actions and requirements');
    expect(html).toContain('Author &amp; fund');
    expect(html).toContain(CAPABILITY_ACTIONS_V1.find((entry) => entry.id === 'product.compile')!.action);
    expect(html).toContain(CAPABILITY_ACTIONS_V1.find((entry) => entry.id === 'market.found')!.action);
    expect(html).toContain(CAPABILITY_ACTIONS_V1.find((entry) => entry.id === 'market.join')!.action);
    expect(html).toContain('Load the selected deployment and market to continue.');
    expect(html).toContain('Where it runs');
    expect(html).toContain('What it does');
    // No disabled control anywhere: every card that cannot be opened links to
    // the page that answers why, which is always reachable.
    expect(html).not.toContain('disabled');
    expect(html.toLowerCase()).not.toContain('transaction unavailable');
    expect(html.toLowerCase()).not.toContain('greyed');
    expect(html).toContain('Program addresses come from Devnet');
    expect(html).toContain('Program overrides · 6 filled from Devnet');
    expect(html).toContain('Filled from the Devnet deployment');
    expect(html).toContain('Optional state coordinates');
    expect(html).not.toContain('Illustrative');
    expect(html).not.toContain('USDC');
  });

  /**
   * The Source state is a DIFFERENT machine at the same finalized floor.
   *
   * A reader shown only the Market's phase has been told half of what was
   * read, and the three answers this line keeps apart are not
   * interchangeable: a decoded state, an account that is not there, and bytes
   * that were refused. Only the last is a defect, and a line that printed the
   * same text for all three would hide it.
   */
  it('says which machine was read beside the Market, and never confuses absent with refused', () => {
    const machine = STATE_MACHINE_RECORDS_V1.find((record) => record.machine === 'source')!;
    const state = machine.states[0]!.state;
    expect(machineObservationTextV1([{ machine: 'source', present: true, state, refusal: null }]))
      .toBe(` · source ${state}`);
    expect(machineObservationTextV1([absentMachineObservationV1('source')])).toBe(' · no source account');
    expect(machineObservationTextV1([{ machine: 'source', present: true, state: null, refusal: 'wrong magic' }]))
      .toBe(' · source refused');
    // An observation this reader never attempted adds nothing, which is what
    // the static render below has and why it must not claim otherwise.
    expect(machineObservationTextV1([])).toBe('');
  });

  it('claims no machine state before anything has been observed', () => {
    const html = renderToStaticMarkup(<MarketWorkbench />);
    for (const record of STATE_MACHINE_RECORDS_V1) {
      expect(html, `${record.machine} appears before any read`).not.toContain(` · ${record.machine} `);
    }
    expect(html).not.toContain('Machine gate');
  });

  it('routes single-field read refusals without guessing at cross-field joins', () => {
    expect(workbenchRefusalFieldV1('Refused: trading program is not executable')).toBe('trading');
    expect(workbenchRefusalFieldV1('Refused: Realm is not owned by the selected Core program')).toBeNull();
    expect(workbenchRefusalFieldV1('Refused: Invalid URL')).toBe('endpoint');
    expect(workbenchRefusalFieldV1('Refused: multiprogram roles must have distinct executable program identities')).toBeNull();
    expect(workbenchRefusalFieldV1('Refused: Realm and Market must have distinct state identities')).toBeNull();
    expect(workbenchRefusalFieldV1('Refused: Realm or Market aliases an executable program role')).toBeNull();
  });

  it('opens the trade stage without synthetic pool or order state', () => {
    const html = renderToStaticMarkup(<MarketWorkbench initialStage="trade" />);
    expect(html).toContain('Trade &amp; provide liquidity');
    expect(html).toContain(CAPABILITY_ACTIONS_V1.find((entry) => entry.id === 'direct.author')!.action);
    expect(html).toContain(CAPABILITY_ACTIONS_V1.find((entry) => entry.id === 'direct.inline')!.action);
    expect(html).toContain('Browser · sign a message');
    expect(html).toContain(CAPABILITY_ACTIONS_V1.find((entry) => entry.id === 'dealer.trade')!.action);
    // An act with no venue names its wall here too, in the same words and
    // with the same citation the census uses.
    expect(html).toContain('Requires:');
    expect(html).toContain('crates/dclutch-trading');
    expect(html).not.toContain('25,000');
    expect(html).not.toContain('Awaiting local chain');
  });

  it('opens the resolution stage', () => {
    const html = renderToStaticMarkup(<MarketWorkbench surface="resolution" initialStage="resolve" />);
    expect(html).toContain('Resolution readiness');
    expect(html).toContain('See what the market needs to reach an outcome');
    expect(html).toContain('Check the market’s resolution status');
    expect(html).toContain('open the next available action');
    expect(html).not.toContain('<strong>Lifecycle readiness</strong>');
  });
});
