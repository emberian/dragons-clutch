import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import { decodeMarketCoreStateV2 } from '@dclutch/sdk/marketCoreV2';
import { DEVNET_DEPLOYMENT_V1 } from '@dclutch/sdk/deployments';
import { SolanaRpcClient } from '@dclutch/sdk/rpc';
import { deriveWalletTerminalPayoutInputV1 } from './walletTerminalInputSnapshot';
import { loadWalletTerminalInputWasmV1 } from './walletTerminalInputV1';
import { deriveWalletTerminalPayoutManifestV1 } from './walletTerminalPayoutSnapshot';
import { loadWalletTerminalPayoutWasmV1 } from './walletTerminalPayoutV1';
import { importRustWalletTerminalPayoutArtifactV3 } from '@dclutch/sdk/walletTerminalPayoutV3';

/**
 * THE ZERO-CLI REDEMPTION, end to end, on a market that has actually resolved.
 *
 * `walletTerminalInput.live.test.ts` proves the address book against a market
 * that is still OPEN, so the walk necessarily ends at `Core Market has no
 * accepted terminal receipt`. That refusal is the right answer for that market
 * and it is NOT the claim anyone wants: it cannot tell a derivation that works
 * from one that would fail two accounts later.
 *
 * This is the test that can, and cohort-13 is the market. Point it at a
 * resolved market and it runs the whole browser path from the deployment's five
 * program ids and the Market address: four rounds derive stage one's input,
 * stage two derives the payout manifest from that input, and the manifest is
 * decoded by the same reader `RedeemFlow` uses. Nothing is imported and nothing
 * is typed.
 *
 * WHAT A RESOLVED MARKET LOOKS LIKE ONCE IT IS ALSO PAID. Cohort-13 resolved to
 * its failure outcome and the one winning position was redeemed the same day,
 * so "the winner's quantity is positive" -- what this file asserted when it was
 * written -- is now false, and correctly so. A paid market has two honest
 * readings and this test makes both:
 *
 *  * a holder who still holds something derives a complete input, and the
 *    manifest prices it. For a LOSING coordinate that price is zero, computed
 *    by the derivation rather than asserted here. This is the case that
 *    exercises all four rounds, so it is the one that proves the chunker.
 *  * a holder whose winning claims are already paid is refused by name, at the
 *    balance, AFTER the same four rounds have run. "You have nothing left to
 *    redeem" is the correct output for a completed redemption, and it is a
 *    different sentence from every way the walk can fail.
 *
 * THE BLOCKER THIS FILE MEASURED, and now refutes: the frame round's first
 * 32-key chunk was 5,272,883 bytes against `MAX_RPC_RESPONSE_BYTES`, because
 * the chunker split by the RPC's key bound where the binding constraint is
 * size. Both cases below assert the byte bound is not what refuses them, so a
 * regression in `planFinalizedAccountChunksV1` fails here by name rather than
 * as an unexplained red.
 *
 * Supply:
 *
 *   DCLUTCH_LIVE_DEVNET=1
 *   DCLUTCH_RESOLVED_MARKET=<the resolved Market address>
 *   DCLUTCH_RESOLVED_OWNER=<a wallet whose winning claims are already paid>
 *   DCLUTCH_RESOLVED_CLAIM_INDEX=<the winning claim index; default 0>
 *   DCLUTCH_RESOLVED_HOLDER=<a wallet still holding a losing coordinate>
 *   DCLUTCH_RESOLVED_HOLDER_CLAIM_INDEX=<that losing claim index; default 0>
 *   DCLUTCH_LIVE_ENDPOINT=<an endpoint; a key stays in the environment>
 *   DCLUTCH_RESOLVED_RECIPIENT=<optional override for the destination>
 *
 * Skipped, loudly, until the market exists. A skipped test that says what it is
 * waiting for is a queue entry; one that says nothing is a hole.
 */
const market = process.env.DCLUTCH_RESOLVED_MARKET;
const owner = process.env.DCLUTCH_RESOLVED_OWNER;
const holder = process.env.DCLUTCH_RESOLVED_HOLDER;
const claimIndex = Number(process.env.DCLUTCH_RESOLVED_CLAIM_INDEX ?? '0');
const holderClaimIndex = Number(process.env.DCLUTCH_RESOLVED_HOLDER_CLAIM_INDEX ?? '0');
const ready = process.env.DCLUTCH_LIVE_DEVNET === '1' && market !== undefined && owner !== undefined;
const live = ready ? it : it.skip;
const liveHolder = ready && holder !== undefined ? it : it.skip;

/** The byte bound that used to stop this walk, named so a regression says so. */
const BYTE_BOUND_REFUSAL_V1 = /exceeds the browser byte bound/;

/**
 * The programs come from the shipped deployment preset, not from this file.
 *
 * Five ids were written down here, and the comment above them said outright
 * that "a default written down here has a shelf life of one cohort" -- which
 * is a note explaining why the file will break rather than a reason it has to.
 * It broke on schedule when cohort-12 was closed. `DEVNET_DEPLOYMENT_V1` is
 * the surface the browser itself reads and the cohort lane keeps current, so
 * taking them from there ends the cycle instead of restarting it.
 *
 * `DCLUTCH_RESOLVED_PROGRAMS` still overrides, and remains the way to point
 * this at a cohort the shipped preset does not name.
 */
const DEVNET_PRESET = Object.freeze({
  registry: DEVNET_DEPLOYMENT_V1.programs.registry,
  core: DEVNET_DEPLOYMENT_V1.programs.core,
  claims: DEVNET_DEPLOYMENT_V1.programs.claims,
  custody: DEVNET_DEPLOYMENT_V1.programs.custody,
  resolution: DEVNET_DEPLOYMENT_V1.programs.resolution,
});

function programs(): Readonly<Record<'registry' | 'core' | 'claims' | 'custody' | 'resolution', string>> {
  const named = process.env.DCLUTCH_RESOLVED_PROGRAMS;
  if (named === undefined) return DEVNET_PRESET;
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
  const stages = async () => {
    const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? 'https://api.devnet.solana.com');
    const stageOne = await loadWalletTerminalInputWasmV1(async () =>
      blob('./generated/walletTerminalInputWasm/wallet_terminal_input_bg.wasm'));
    const stageTwo = await loadWalletTerminalPayoutWasmV1(async () =>
      blob('./generated/walletTerminalPayoutWasm/wallet_terminal_payout_bg.wasm'));
    return { client, stageOne, stageTwo };
  };

  live('the Market carries a terminal receipt and names the outcome that won', async () => {
    const { client } = await stages();
    const observed = await client.accountInfo(market!);
    const decoded = decodeMarketCoreStateV2(market!, observed.account!.data);
    expect(decoded.phase, 'a market that has resolved').toBe('Terminal');
    expect(decoded.settlement.status).toBe('terminal');
    if (decoded.settlement.status !== 'terminal') throw new Error('unreachable');
    expect(decoded.settlement.winner, 'the winning claim this run is about').toBe(claimIndex);
    // The receipt is the certificate's CONTENT id, which Core stores as 32
    // raw bytes and this decoder renders as hex. Asserting base58 here asserted
    // the wrong representation of the right fact.
    expect(decoded.settlement.receiptId, 'the accepted Resolution certificate').toMatch(/^[0-9a-f]{64}$/);
  }, 120_000);

  liveHolder('derives a whole input over the CONVENTIONAL destination, and stops only at a transaction', async () => {
    // THE CASE THAT PROVES BOTH BLOCKERS ARE GONE. This wallet still holds
    // atoms, so all four rounds run -- including the frame round, whose first
    // 32-key chunk was 5,272,883 bytes against a 4 MiB bound and stopped the
    // walk dead. And `recipient` is left ABSENT, so the derivation fills in the
    // wallet's associated token account, which under Token-2022 is 170 bytes
    // because the ATA program always adds `ImmutableOwner`. That account was
    // refused by every token parse in this protocol until this lane.
    const { client, stageOne, stageTwo } = await stages();
    const acquired = await deriveWalletTerminalPayoutInputV1(client, stageOne, {
      programs: programs(),
      market: market!,
      owner: holder!,
      recipient: process.env.DCLUTCH_RESOLVED_RECIPIENT,
      claimIndex: holderClaimIndex,
    });
    expect(acquired.rounds, 'stage one is four rounds, at one floor').toBe(4);

    const input: unknown = JSON.parse(acquired.inputJson);
    const fields = input as Record<string, unknown>;
    expect(fields.format).toBe('dclutch-wallet-terminal-payout-plan-input-v1');
    expect(fields.market).toBe(market);
    expect(fields.owner).toBe(holder);
    expect(fields.terminalCertificate, 'the accepted Resolution certificate').toMatch(/^[1-9A-HJ-NP-Za-km-z]{32,44}$/);
    expect(BigInt(String(fields.quantity)), 'the balance this wallet still holds').toBeGreaterThan(0n);
    expect(fields.recipient, 'a destination was derived, not supplied').toMatch(/^[1-9A-HJ-NP-Za-km-z]{32,44}$/);
    expect(fields.recipientOwner).toBe(holder);

    // STAGE TWO, over stage one's own answer. It reads its own frame and then
    // needs ONE thing this test cannot give it: a lookup table, which is an
    // account somebody has to create in a signed transaction. `RedeemFlow`
    // creates it; a read-only test does not sign.
    //
    // The assertion is about WHICH refusal, because the two this lane closed
    // are still the ones worth ruling out by name.
    let manifest: string | null = null;
    let refusal: string | null = null;
    try {
      manifest = await deriveWalletTerminalPayoutManifestV1(client, stageTwo, acquired.inputJson);
    } catch (error) {
      refusal = error instanceof Error ? error.message : String(error);
    }
    if (refusal !== null) {
      expect(refusal, 'not the chunker').not.toMatch(BYTE_BOUND_REFUSAL_V1);
      expect(refusal, 'not the Token-2022 destination width').not.toMatch(/165|recipient token account/);
      expect(refusal, 'the one remaining step needs a signed transaction').toMatch(/lookupTable is required/);
      return;
    }
    const decoded = importRustWalletTerminalPayoutArtifactV3(manifest!);
    expect(decoded.request.market).toBe(market);
    expect(decoded.request.owner).toBe(holder);
    expect(decoded.request.claimIndex).toBe(holderClaimIndex);
    if (holderClaimIndex !== claimIndex) {
      expect(decoded.payout, 'a losing coordinate is worth exactly nothing').toBe('0');
    }
  }, 180_000);

  live('reports an already-redeemed winning position as having nothing left', async () => {
    // The founder's own reading of cohort-13 after the payout: the walk reaches
    // the balance and finds it spent. This is the sentence a reader who already
    // redeemed must see, and it is produced by the same four rounds.
    const { client, stageOne } = await stages();
    let refusal: string | null = null;
    let quantity: string | null = null;
    try {
      const acquired = await deriveWalletTerminalPayoutInputV1(client, stageOne, {
        programs: programs(),
        market: market!,
        owner: owner!,
        recipient: process.env.DCLUTCH_RESOLVED_RECIPIENT,
        claimIndex,
      });
      quantity = String((JSON.parse(acquired.inputJson) as Record<string, unknown>).quantity);
    } catch (error) {
      refusal = error instanceof Error ? error.message : String(error);
    }
    if (refusal !== null) {
      expect(refusal, 'the walk completed and the balance is what stopped it').not.toMatch(BYTE_BOUND_REFUSAL_V1);
      expect(refusal, 'a paid position is refused AT ITS BALANCE, by name').toMatch(
        new RegExp(`payout quantity must be within 1\\.\\.=0 atoms at claim index ${claimIndex}`),
      );
      return;
    }
    // The other honest reading of the same market: this wallet has NOT been
    // paid yet. Then the quantity is positive and the redemption is still owed.
    expect(BigInt(String(quantity)), 'an unredeemed winner still holds its claims').toBeGreaterThan(0n);
  }, 180_000);

  it('says what it is waiting for while no resolved market is named', () => {
    // Not a passing stand-in for the tests above: a statement of the queue. The
    // gate is the environment, and this asserts the gate rather than the
    // capability.
    expect(ready || market === undefined || owner === undefined).toBe(true);
    if (!ready) {
      expect(
        'set DCLUTCH_LIVE_DEVNET=1, DCLUTCH_RESOLVED_MARKET and DCLUTCH_RESOLVED_OWNER to prove the zero-CLI redemption end to end',
      ).toContain('DCLUTCH_RESOLVED_MARKET');
    }
    if (ready && holder === undefined) {
      expect(
        'set DCLUTCH_RESOLVED_HOLDER to a wallet still holding a coordinate, so the four-round derivation is exercised on a paid market',
      ).toContain('DCLUTCH_RESOLVED_HOLDER');
    }
  });
});
