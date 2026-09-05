import { describe, expect, it } from 'vitest';

import { CORE_STATE_GENERATION_OFFSET } from './generated/coreFound';
import { CAPABILITY_ROOT_HEADER_BYTES_V1 } from './generated/directInlineV3';
import { routeMachineStatesV1 } from './generated/marketPhaseAdmissionV1';
import { u64 } from './bytes';
import { DEVNET_DEPLOYMENT_V1 } from './deployments';
import { PUBLIC_DEVNET_CUT_V1 } from './publicCutStaging';
import { SolanaRpcClient } from './rpc';
import {
  absentMachineObservationV1,
  decodeDirectRootStateV1,
  decodeFundingLedgerSlotV2,
  decodeMachineStateV1,
  decodeSourceResolutionStateV2,
  machineObservationV1,
  routeMachineVerdictsV1,
  sourceResolutionStateAddressV2,
} from './stateMachines';

/**
 * The machine decoders against the chain that writes them.
 *
 * `stateMachines.test.ts` runs the same decoders against bytes captured off
 * this chain on 2026-09-04. This one re-reads them, so the captured vector
 * cannot quietly become a description of a chain that has moved on, and it
 * exercises the one thing a fixture cannot: the ADDRESS. The Source state is
 * derived here from a Market and a generation and nothing else, and the test
 * passes only if the account at the derived address decodes.
 *
 * THE COORDINATES MOVE WITH THE COHORT, and the cost of not doing that is why
 * this paragraph exists. They were cohort-15's literals, and the day cohort-16
 * landed two cases went red naming an ABSENT account -- a Source whose
 * lifecycle its own cohort had closed -- while two others went on passing
 * against accounts owned by a program that no longer exists, which is worse:
 * bytes a dead program wrote still decode, so a stale coordinate is invisible
 * exactly where the decoder is doing its job. The Market comes from the public
 * cut now; the two funding ledgers are cohort-16's own, from
 * `docs/evidence/COHORT16_DEPLOYED_SEALED_2026_09_05.md`'s addendum, and each
 * is asserted to be owned by the program `DEVNET_DEPLOYMENT_V1` names, so the
 * next boundary makes them red rather than quietly historical.
 *
 * Every assertion is the AGREEMENT between what was decoded and what the
 * published gate says, never a literal state: a Source that advances changes
 * which branch runs and not whether the test passes.
 *
 * WHAT COHORT-16 DOES NOT HAVE, said once here. Its featured market could not
 * be activated -- the deployed Direct release wants a two-account funding frame
 * and this market's capability dependency edges need a wider one -- so there is
 * no activation root, and it is the only market on the cohort, so there is no
 * second Source to disagree with the first. The two cases that needed those are
 * below and each says so in its own words rather than being deleted.
 *
 * Gated on `DCLUTCH_LIVE_DEVNET=1`. Five account reads.
 */

const RESOLUTION_PROGRAM = DEVNET_DEPLOYMENT_V1.programs.resolution;
const TRADING_PROGRAM = DEVNET_DEPLOYMENT_V1.programs.trading;
const CORE_PROGRAM = DEVNET_DEPLOYMENT_V1.programs.core;

/** The featured Market, out of the cut. Never a literal. */
const FEATURED_MARKET = PUBLIC_DEVNET_CUT_V1.market ?? '';
/** Cohort-16's Trading capability funding ledger, selected mask 0x0001. */
const TRADING_LEDGER = 'GJwPzPdz5ppCD8sz3ymaZvcabsmeBSKNy5f7GFX2mqeh';
/** Its Resolution companion, selected mask 0x000e -- the three compartments. */
const RESOLUTION_LEDGER = 'DtvxF2xgFvnNuCgn7uDuKcErWfVHvJatvusm9ZDRShrd';

/**
 * The capability-root header the Direct family's mutable tail follows.
 *
 * Imported rather than restated: the header's width is the Direct ABI module's
 * fact, and a second copy of it here is exactly the hand mirror this lane's
 * generated table exists to stop.
 */
const CAPABILITY_ROOT_HEADER_BYTES = CAPABILITY_ROOT_HEADER_BYTES_V1;

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' && FEATURED_MARKET !== '' ? it : it.skip;

const client = () => new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? 'https://api.devnet.solana.com');

async function read(address: string, owner: string): Promise<Uint8Array> {
  const observation = await client().accountInfo(address);
  expect(observation.account, `no account at ${address}`).not.toBeNull();
  expect(observation.account!.owner, `${address} is not owned by ${owner}`).toBe(owner);
  return observation.account!.data;
}

describe('live devnet state machines', () => {
  /**
   * The Direct root's tail, read and parsed rather than measured.
   *
   * `directHotChain.ts` checks this account's LENGTH against `232 + 24` and
   * never reads the 24. Here they are read: the phase decides whether a new
   * maker nonce can be consumed, and the count decides whether the global root
   * can be closed. Both gates are asserted against the census's own sets.
   */
  live.skip('parses the Direct root tail and answers both of its routes', async () => {
    // NO SUBJECT ON THIS COHORT, and skipped rather than pointed at cohort-15's
    // root, which is still readable and is owned by a closed program. Cohort-16
    // could not activate its market, so no Direct capability root exists; the
    // case below asserts that absence positively so this skip cannot be the
    // only trace of it. `capabilitySelectedGate.test.ts` decodes the same tail
    // offline from a captured devnet vector.
    const account = await read('', TRADING_PROGRAM);
    const tail = account.subarray(CAPABILITY_ROOT_HEADER_BYTES);
    const decode = decodeDirectRootStateV1(tail);
    expect(decode.status, decode.status === 'refused' ? decode.reason : '').toBe('decoded');
    if (decode.status !== 'decoded') return;

    const observations = [machineObservationV1(decode)];
    // `direct_token_setup_v1` admits Open; `direct_close_maker_v1` admits
    // Retiring. They are disjoint by construction (the two sets partition the
    // machine), so exactly one of the two verdicts is `admitted` whatever the
    // root's phase -- which is the assertion, rather than a phase literal.
    const setup = routeMachineVerdictsV1('trading/direct_token_setup_v1::process_direct_token_setup_v1', observations);
    const close = routeMachineVerdictsV1('trading/direct_close_maker_v1::process_direct_close_maker_v1', observations);
    expect(setup[0]!.states).toEqual(['Open']);
    expect(close[0]!.states).toEqual(['Retiring']);
    const admitted = [setup[0]!.verdict, close[0]!.verdict].filter((verdict) => verdict === 'admitted');
    expect(admitted).toHaveLength(1);
    expect(setup[0]!.verdict === 'admitted' ? 'Open' : 'Retiring').toBe(decode.state);
    // The refusal names the machine, its set and what was observed.
    const refused = setup[0]!.verdict === 'excluded' ? setup[0]! : close[0]!;
    expect(refused.reason).toContain('direct-root');
    expect(refused.reason).toContain(decode.state);

    // The count the phase alone cannot answer for: `require_closable` is the
    // conjunction, so a Retiring root with makers still open refuses too.
    expect(typeof decode.counters.openMakerRootCount).toBe('bigint');
  }, 60_000);

  /**
   * Two Sources, at addresses DERIVED from their Markets, in two states.
   *
   * Nothing here is given a Source address. Each is computed from its Market
   * and the generation read out of that Market's own bytes, so a wrong seed
   * order or a wrong domain is an account that does not exist rather than a
   * silently different one.
   */
  live('derives the Source state from its Market, and refuses a neighbouring generation', async () => {
    const core = await read(FEATURED_MARKET, CORE_PROGRAM);
    const generation = u64(core, CORE_STATE_GENERATION_OFFSET);
    const address = sourceResolutionStateAddressV2(FEATURED_MARKET, generation, RESOLUTION_PROGRAM);
    const decode = decodeSourceResolutionStateV2(await read(address, RESOLUTION_PROGRAM));
    expect(decode.status, decode.status === 'refused' ? `${address}: ${decode.reason}` : '').toBe('decoded');

    // THE CONTROL A SINGLE DECODE CANNOT GIVE. It used to be two Markets of one
    // cohort whose Sources disagreed; cohort-16 has one Market, so the control
    // moves onto the SEED, which is what this case is uniquely about anyway. A
    // derivation that ignored the generation would hand back the same account
    // for generation+1, and it must hand back one that does not exist.
    const neighbour = sourceResolutionStateAddressV2(FEATURED_MARKET, generation + 1n, RESOLUTION_PROGRAM);
    expect(neighbour).not.toBe(address);
    expect((await client().accountInfo(neighbour)).account, `${neighbour} exists, so the generation seed is doing no work`).toBeNull();
  }, 60_000);

  /** The sponsored capture gate, answered from a decoded Source. */
  live('answers the sponsored capture gate from the decoded Source, both ways', async () => {
    const route = 'resolution/process_capture#Capture';
    expect(routeMachineStatesV1(route, 'source')).toEqual(['Primary']);
    for (const market of [FEATURED_MARKET]) {
      const core = await read(market, CORE_PROGRAM);
      const address = sourceResolutionStateAddressV2(market, u64(core, CORE_STATE_GENERATION_OFFSET), RESOLUTION_PROGRAM);
      const decode = decodeSourceResolutionStateV2(await read(address, RESOLUTION_PROGRAM));
      if (decode.status !== 'decoded') throw new Error(`${address} did not decode`);
      const [verdict] = routeMachineVerdictsV1(route, [machineObservationV1(decode)]);
      expect(verdict!.verdict).toBe(decode.state === 'Primary' ? 'admitted' : 'excluded');
      expect(verdict!.observed).toBe(decode.state);
      if (verdict!.verdict === 'excluded') {
        expect(verdict!.reason).toContain('source Primary');
        expect(verdict!.reason).toContain(decode.state);
      }
    }
  }, 90_000);

  /**
   * Every selected slot of BOTH live funding ledgers, by row.
   *
   * Two ledgers, not one, and they are the pair cohort-16's founding produced:
   * the Trading ledger carries the one selected entry (mask `0x0001`) and its
   * Resolution companion carries the three compartments (mask `0x000e`), so
   * they are different widths and a row count that came from the decoder rather
   * than from the account would show up as one of them disagreeing.
   */
  live('decodes every slot of both live funding ledgers', async () => {
    const widths: number[] = [];
    for (const [ledger, owner] of [[TRADING_LEDGER, TRADING_PROGRAM], [RESOLUTION_LEDGER, RESOLUTION_PROGRAM]] as const) {
      const bytes = await read(ledger, owner);
      expect((bytes.length - 48) % 72, ledger).toBe(0);
      const rows = (bytes.length - 48) / 72;
      expect(rows, ledger).toBeGreaterThan(0);
      for (let row = 0; row < rows; row += 1) {
        const decode = decodeFundingLedgerSlotV2(bytes, row);
        expect(decode.status, decode.status === 'refused' ? `${ledger} row ${row}: ${decode.reason}` : '').toBe('decoded');
        if (decode.status === 'decoded') expect(['Pending', 'Active', 'Closed']).toContain(decode.state);
      }
      // One past the end is an absence, not a state.
      expect(decodeFundingLedgerSlotV2(bytes, rows).status, ledger).toBe('refused');
      widths.push(rows);
    }
    expect(new Set(widths).size, 'both ledgers hold the same number of slots, so this pair is not the control it claims').toBe(2);
  }, 60_000);

  /**
   * The negative control: a machine whose account this cohort does not have.
   *
   * `series-ticket`, `dealer-checkpoint`, `dealer-reservation`,
   * `projected-custody` and `dealer-root` have no instance on this cohort, so a
   * reader must say `unobserved` and NEVER admit. This reads no account for
   * that half -- it asserts that the absent answer is the one the gate
   * produces, which is the distinction the whole `needs-chain` path exists to
   * keep -- and then reads a real record to prove the OTHER half: a machine
   * decoded from the wrong record's bytes is refused rather than admitted.
   *
   * AND IT CARRIES THE ABSENCE THE SKIPPED CASE ABOVE WOULD OTHERWISE HIDE.
   * `direct-root` is on this list for cohort-16 for the first time: the
   * activation that would have created one refused, so there is no Direct
   * capability root, and the reader must answer `unobserved` about it too.
   */
  live('never admits a machine whose account this cohort does not have', async () => {
    const route = 'core/series_consume::process';
    for (const observations of [[], [absentMachineObservationV1('series-ticket')]]) {
      const verdicts = routeMachineVerdictsV1(route, observations);
      expect(verdicts.length).toBeGreaterThan(0);
      expect(verdicts.every((verdict) => verdict.verdict === 'unobserved')).toBe(true);
    }
    // THE DIRECT ROOT IS ABSENT ON THIS COHORT, positively. Activation refused,
    // so nothing created one -- and a reader must say `unobserved` about it
    // rather than admitting a route that needs it.
    for (const verdict of routeMachineVerdictsV1('trading/direct_token_setup_v1::process_direct_token_setup_v1', [absentMachineObservationV1('direct-root')])) {
      expect(verdict.verdict).toBe('unobserved');
    }

    // And a machine read with the wrong record's bytes is refused, not
    // admitted. The Trading funding ledger is a real record of this cohort and
    // is not a series ticket.
    const account = await read(TRADING_LEDGER, TRADING_PROGRAM);
    expect(decodeMachineStateV1('series-ticket', account.subarray(CAPABILITY_ROOT_HEADER_BYTES)).status).toBe('refused');
  }, 60_000);
});
