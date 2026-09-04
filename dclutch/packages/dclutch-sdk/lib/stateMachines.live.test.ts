import { describe, expect, it } from 'vitest';

import { CORE_STATE_GENERATION_OFFSET } from './generated/coreFound';
import { CAPABILITY_ROOT_HEADER_BYTES_V1 } from './generated/directInlineV3';
import { routeMachineStatesV1 } from './generated/marketPhaseAdmissionV1';
import { u64 } from './bytes';
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
 * The coordinates are cohort-15's, from
 * `docs/evidence/COHORT15_DEPLOYED_SEALED_FOUNDED_CAPTURED_2026_09_04.md`
 * sections 3, 8 and its addendum. They are named rather than taken from the
 * public cut because the cut follows whatever is featured, and these cases
 * want two Markets whose Sources are in DIFFERENT states.
 *
 * Every assertion is the AGREEMENT between what was decoded and what the
 * published gate says, never a literal state: a Source that advances changes
 * which branch runs and not whether the test passes.
 *
 * Gated on `DCLUTCH_LIVE_DEVNET=1`. Six account reads.
 */

const COHORT15_RESOLUTION_PROGRAM = '24AkUjtXg61La45u7KTge8u4dKpVqkzirmzycVyckFgn';
const COHORT15_TRADING_PROGRAM = '3gBSSjYwSC4phutpGKRkMhrnCDVzHu6kfQ3L4jLf2UmG';
const COHORT15_CORE_PROGRAM = '7hGerMC6Wj742FVTyiF9PhRnGSBzbee7TMZ6sUytsmFr';

/** The founded Open Market, section 8. */
const COHORT15_OPEN_MARKET = '3QytL1bBMtCvRoXWR5h7MgutRBZqtv7emUVubEo5a4T2';
/** The General Market of the same cohort, the addendum. */
const COHORT15_GENERAL_MARKET = '6aqy89GhhXFtDbawC5ors4HLkGvzdHC4R26TXTaaXRKj';
/** The activation root section 8 names; its Direct family tail follows the header. */
const COHORT15_ACTIVATION_ROOT = 'FUJ9pNukWRb5658ysiDP5gz9gF3Hx8c4cU2mUrvELAWG';
/** The Trading capability funding ledger of the same cohort. */
const COHORT15_TRADING_LEDGER = '7c8Y9rTjSPPn9rAcwoQGicfrpJEXhaMafmZn2KXgvjGF';

/**
 * The capability-root header the Direct family's mutable tail follows.
 *
 * Imported rather than restated: the header's width is the Direct ABI module's
 * fact, and a second copy of it here is exactly the hand mirror this lane's
 * generated table exists to stop.
 */
const CAPABILITY_ROOT_HEADER_BYTES = CAPABILITY_ROOT_HEADER_BYTES_V1;

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' ? it : it.skip;

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
  live('parses the Direct root tail and answers both of its routes', async () => {
    const account = await read(COHORT15_ACTIVATION_ROOT, COHORT15_TRADING_PROGRAM);
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
  live('derives two Source states from their Markets and finds them in different states', async () => {
    const states = new Map<string, string>();
    for (const market of [COHORT15_OPEN_MARKET, COHORT15_GENERAL_MARKET]) {
      const core = await read(market, COHORT15_CORE_PROGRAM);
      const generation = u64(core, CORE_STATE_GENERATION_OFFSET);
      const address = sourceResolutionStateAddressV2(market, generation, COHORT15_RESOLUTION_PROGRAM);
      const bytes = await read(address, COHORT15_RESOLUTION_PROGRAM);
      const decode = decodeSourceResolutionStateV2(bytes);
      expect(decode.status, decode.status === 'refused' ? `${address}: ${decode.reason}` : '').toBe('decoded');
      if (decode.status !== 'decoded') return;
      states.set(market, decode.state);
    }
    expect(states.size).toBe(2);
    // The control a single read cannot give: two Sources of one cohort that do
    // not agree, so a decoder that returned a constant could not pass.
    expect(new Set(states.values()).size).toBe(2);
  }, 60_000);

  /** The sponsored capture gate, answered from a decoded Source. */
  live('answers the sponsored capture gate from the decoded Source, both ways', async () => {
    const route = 'resolution/process_capture#Capture';
    expect(routeMachineStatesV1(route, 'source')).toEqual(['Primary']);
    for (const market of [COHORT15_OPEN_MARKET, COHORT15_GENERAL_MARKET]) {
      const core = await read(market, COHORT15_CORE_PROGRAM);
      const address = sourceResolutionStateAddressV2(market, u64(core, CORE_STATE_GENERATION_OFFSET), COHORT15_RESOLUTION_PROGRAM);
      const decode = decodeSourceResolutionStateV2(await read(address, COHORT15_RESOLUTION_PROGRAM));
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

  /** Every selected slot of the live Trading funding ledger, by row. */
  live('decodes every slot of the live Trading funding ledger', async () => {
    const bytes = await read(COHORT15_TRADING_LEDGER, COHORT15_TRADING_PROGRAM);
    expect((bytes.length - 48) % 72).toBe(0);
    const rows = (bytes.length - 48) / 72;
    expect(rows).toBeGreaterThan(0);
    for (let row = 0; row < rows; row += 1) {
      const decode = decodeFundingLedgerSlotV2(bytes, row);
      expect(decode.status, decode.status === 'refused' ? decode.reason : '').toBe('decoded');
      if (decode.status === 'decoded') expect(['Pending', 'Active', 'Closed']).toContain(decode.state);
    }
    // One past the end is an absence, not a state.
    expect(decodeFundingLedgerSlotV2(bytes, rows).status).toBe('refused');
  }, 60_000);

  /**
   * The negative control: a machine whose account this cohort does not have.
   *
   * `series-ticket`, `dealer-checkpoint`, `dealer-reservation`,
   * `projected-custody` and `dealer-root` have no cohort-15 instance, so a
   * reader must say `unobserved` and NEVER admit. This reads no account -- it
   * asserts that the absent answer is the one the gate produces, which is the
   * distinction the whole `needs-chain` path exists to keep.
   */
  live('never admits a machine whose account this cohort does not have', async () => {
    const route = 'core/series_consume::process';
    for (const observations of [[], [absentMachineObservationV1('series-ticket')]]) {
      const verdicts = routeMachineVerdictsV1(route, observations);
      expect(verdicts.length).toBeGreaterThan(0);
      expect(verdicts.every((verdict) => verdict.verdict === 'unobserved')).toBe(true);
    }
    // And a machine read with the wrong record's bytes is refused, not admitted.
    const account = await read(COHORT15_ACTIVATION_ROOT, COHORT15_TRADING_PROGRAM);
    expect(decodeMachineStateV1('series-ticket', account.subarray(CAPABILITY_ROOT_HEADER_BYTES)).status).toBe('refused');
  }, 60_000);
});
