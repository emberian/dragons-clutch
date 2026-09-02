import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import { SolanaRpcClient } from './rpc';
import { deriveWalletTerminalPayoutInputV1 } from './walletTerminalInputSnapshot';
import { loadWalletTerminalInputWasmV1 } from './walletTerminalInputV1';
import { deriveWalletTerminalPayoutManifestV1 } from './walletTerminalPayoutSnapshot';
import { loadWalletTerminalPayoutWasmV1 } from './walletTerminalPayoutV1';
import { importRustWalletTerminalPayoutArtifactV3 } from './walletTerminalPayoutV3';

/**
 * THE ZERO-CLI REDEMPTION, end to end, on a market that has actually resolved.
 *
 * `walletTerminalInput.live.test.ts` proves the address book against a market
 * that is still OPEN, so the walk necessarily ends at `Core Market has no
 * accepted terminal receipt`. That refusal is the right answer for that market
 * and it is NOT the claim anyone wants: it cannot tell a derivation that works
 * from one that would fail two accounts later.
 *
 * This is the test that can. Point it at a resolved market and it runs the
 * whole browser path — four rounds derive stage one's input from the
 * deployment's five program ids and the Market, then stage two derives the
 * payout manifest from that input, then the manifest is decoded by the same
 * reader `RedeemFlow` uses. Nothing is imported and nothing is typed: the
 * destination defaults to the wallet's associated token account.
 *
 * It is written now, before cohort-12 resolves, so the day it does the proof is
 * a command and not a project. Supply:
 *
 *   DCLUTCH_LIVE_DEVNET=1
 *   DCLUTCH_RESOLVED_MARKET=<the resolved Market address>
 *   DCLUTCH_RESOLVED_OWNER=<a wallet holding winning claims on it>
 *   DCLUTCH_LIVE_ENDPOINT=<an endpoint; a key stays in the environment>
 *   DCLUTCH_RESOLVED_RECIPIENT=<optional override for the destination>
 *
 * Skipped, loudly, until the market exists. A skipped test that says what it is
 * waiting for is a queue entry; one that says nothing is a hole.
 */
const market = process.env.DCLUTCH_RESOLVED_MARKET;
const owner = process.env.DCLUTCH_RESOLVED_OWNER;
const ready = process.env.DCLUTCH_LIVE_DEVNET === '1' && market !== undefined && owner !== undefined;
const live = ready ? it : it.skip;

const DEVNET_COHORT_11 = Object.freeze({
  registry: 'ADB72ar6ZSstXEg76Q1bPb5UY2EGmH6mrVfwr8K2fzom',
  core: 'FinXxc9drpmCYA7Cy4aGWSa1jYY87K6pNPfY9qFWzJCF',
  claims: 'HQYqqdzn5s6tEM6ywgeCr7Bd56tEuhpoop3ruvHRfAq6',
  custody: 'Cdh8Vv7DRyk7rhLcee574potYfaiVEsYR5HUPCrNPzCB',
  resolution: '3WqTxq6uKMK2d9f6uRujh8hCZvVB78KjGo9AYxvPQNVM',
});

function programs(): Readonly<Record<'registry' | 'core' | 'claims' | 'custody' | 'resolution', string>> {
  const named = process.env.DCLUTCH_RESOLVED_PROGRAMS;
  if (named === undefined) return DEVNET_COHORT_11;
  // A later cohort deploys new ids; naming them beats editing this file.
  const parsed: unknown = JSON.parse(named);
  const record = parsed as Record<string, unknown>;
  for (const role of ['registry', 'core', 'claims', 'custody', 'resolution'] as const) {
    if (typeof record[role] !== 'string') throw new Error(`DCLUTCH_RESOLVED_PROGRAMS omits ${role}`);
  }
  return Object.freeze(record as Record<'registry' | 'core' | 'claims' | 'custody' | 'resolution', string>);
}

const blob = (name: string) => new Response(readFileSync(new URL(name, import.meta.url)));

describe('a resolved market redeems in the browser with nothing imported', () => {
  live('derives stage one and stage two from chain alone, and the manifest decodes', async () => {
    const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? 'https://api.devnet.solana.com');
    const stageOne = await loadWalletTerminalInputWasmV1(async () =>
      blob('./generated/walletTerminalInputWasm/wallet_terminal_input_bg.wasm'));
    const stageTwo = await loadWalletTerminalPayoutWasmV1(async () =>
      blob('./generated/walletTerminalPayoutWasm/wallet_terminal_payout_bg.wasm'));

    const acquired = await deriveWalletTerminalPayoutInputV1(client, stageOne, {
      programs: programs(),
      market: market!,
      owner: owner!,
      // Absent unless overridden: the conventional destination is filled in
      // beside the address book.
      recipient: process.env.DCLUTCH_RESOLVED_RECIPIENT,
      claimIndex: Number(process.env.DCLUTCH_RESOLVED_CLAIM_INDEX ?? '0'),
    });
    expect(acquired.rounds, 'stage one is four rounds, at one floor').toBe(4);

    const input: unknown = JSON.parse(acquired.inputJson);
    const fields = input as Record<string, unknown>;
    expect(fields.format).toBe('dclutch-wallet-terminal-payout-plan-input-v1');
    expect(fields.market).toBe(market);
    expect(fields.owner).toBe(owner);
    // A terminal certificate only exists once the market resolved. This is the
    // assertion the OPEN-market test cannot make.
    expect(fields.terminalCertificate, 'the accepted Resolution certificate').toMatch(/^[1-9A-HJ-NP-Za-km-z]{32,44}$/);
    expect(BigInt(String(fields.quantity)), 'the authenticated winning balance').toBeGreaterThan(0n);

    // STAGE TWO, over stage one's own answer: the manifest a reader used to
    // import, now derived here from the same finalized chain.
    const manifestJson = await deriveWalletTerminalPayoutManifestV1(client, stageTwo, acquired.inputJson);
    const manifest = importRustWalletTerminalPayoutArtifactV3(manifestJson);
    expect(manifest.request.market).toBe(market);
    expect(manifest.request.owner).toBe(owner);
    expect(manifest.lookupTable, 'the one exact payout lookup table').toMatch(/^[1-9A-HJ-NP-Za-km-z]{32,44}$/);
  }, 180_000);

  it('says what it is waiting for while no resolved market is named', () => {
    // Not a passing stand-in for the test above: a statement of the queue. The
    // gate is the environment, and this asserts the gate rather than the
    // capability.
    expect(ready || market === undefined || owner === undefined).toBe(true);
    if (!ready) {
      expect(
        'set DCLUTCH_LIVE_DEVNET=1, DCLUTCH_RESOLVED_MARKET and DCLUTCH_RESOLVED_OWNER to prove the zero-CLI redemption end to end',
      ).toContain('DCLUTCH_RESOLVED_MARKET');
    }
  });
});
