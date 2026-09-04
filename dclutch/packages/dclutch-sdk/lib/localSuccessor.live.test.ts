import { describe, expect, it } from 'vitest';

import machineVectors from '../fixtures/state-machines.devnet.json';
import { LOCAL_SUCCESSOR_CHECKPOINT, discoverLocalSuccessor, parseSuccessorAccount } from './localSuccessor';
import { SolanaRpcClient } from './rpc';

const live = process.env.DCLUTCH_LIVE_SUCCESSOR_RPC === '1' ? it : it.skip;

describe('live immutable successor validator', () => {
  live('reacquires the complete finalized checkpoint without unexpected program state', async () => {
    const snapshot = await discoverLocalSuccessor(new SolanaRpcClient(LOCAL_SUCCESSOR_CHECKPOINT.network.rpc_url));
    expect(snapshot.facts.genesisHash).toBe(LOCAL_SUCCESSOR_CHECKPOINT.network.genesis_hash);
    expect(snapshot.exactAccounts).toBe(33);
    expect(snapshot.transactionCreatedAccounts).toBe(7);
    expect(snapshot.missingProgramAccounts).toEqual([]);
    expect(snapshot.unexpectedProgramAccounts).toEqual([]);
    expect(snapshot.transactions).toHaveLength(9);
    expect(snapshot.transactions.every((transaction) => transaction.rpcStatus !== 'mismatch')).toBe(true);
    expect(snapshot.rollbackCurrent).toBe(true);
  });
});

/**
 * The two records this surface no longer decodes in its own words.
 *
 * `localSuccessor.test.ts` runs `parseSuccessorAccount` over cohort-15 bytes
 * captured on 2026-09-04 and committed as `fixtures/state-machines.devnet.json`.
 * This re-reads the same addresses from devnet, so that vector cannot quietly
 * become a description of a chain that has moved on, and it exercises the one
 * thing the fixture cannot: that the accounts are still THERE, still owned by
 * the program the vector names, and still of the generation this client reads.
 *
 * Nothing here asserts a state literal. A Source that advances from Primary to
 * Resolved must not turn this red -- what is asserted is the AGREEMENT between
 * the committed vector and the chain about which record generation these
 * addresses hold, which is the fact the surface was wrong about before.
 *
 * Gated on `DCLUTCH_LIVE_DEVNET=1`. Two account reads.
 */
describe('live devnet records through the successor surface', () => {
  type MachineVector = Readonly<{ machine: string; address: string; owner: string; recordHex: string }>;
  const VECTORS = machineVectors.records as ReadonlyArray<MachineVector>;
  const devnet = process.env.DCLUTCH_LIVE_DEVNET === '1' ? it : it.skip;
  const CASES = [
    { machine: 'source', name: 'lifecycle.state', kind: 'Source resolution state' },
    { machine: 'funding-ledger', name: 'lifecycle.funding.failure', kind: 'capability funding ledger' },
  ] as const;

  for (const testCase of CASES) {
    devnet(`reads cohort-15 ${testCase.machine} off devnet through parseSuccessorAccount`, async () => {
      const vector = VECTORS.find((entry) => entry.machine === testCase.machine);
      expect(vector, `no committed ${testCase.machine} vector`).toBeDefined();
      const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? 'https://api.devnet.solana.com');
      const observation = await client.accountInfo(vector!.address);
      expect(observation.account, `no account at ${vector!.address}`).not.toBeNull();
      expect(observation.account!.owner).toBe(vector!.owner);

      const chain = parseSuccessorAccount(testCase.name, observation.account!);
      const committed = parseSuccessorAccount(testCase.name, Object.freeze({
        ...observation.account!,
        data: Uint8Array.from(vector!.recordHex.match(/../g) ?? [], (byte) => Number.parseInt(byte, 16)),
      }));

      expect(chain.kind).toBe(testCase.kind);
      const named = (parsed: typeof chain, label: string) => parsed.facts.find((entry) => entry.label === label)?.value;
      // The generation, not the state: these two must agree even when the
      // machine has advanced since the vector was captured.
      expect(named(chain, 'record')).toBe(named(committed, 'record'));
      expect(named(chain, 'magic')).toBe(named(committed, 'magic'));
      expect(chain.headline.length).toBeGreaterThan(0);
      expect(named(chain, 'wire tag')).toMatch(/^[0-9]+$/);
    });
  }
});
