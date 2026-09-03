import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { DEVNET_DEPLOYMENT_V1 } from './deployments';
import {
  TOKEN_ACCOUNT_BYTES_V1,
  TOKEN_ACCOUNT_IMMUTABLE_OWNER_BYTES_V1,
  TOKEN_ACCOUNT_IMMUTABLE_OWNER_SUFFIX_V1,
} from './generated/walletTerminalPayoutV3';
import { TOKEN_2022_PROGRAM_ID, decodeToken2022BehaviorAccountV2 } from './rationalTokenV2';
import { SolanaRpcClient } from './rpc';
import { admitBaseOrImmutableOwnerTokenAccountV1 } from './tokenAccountAdmissionV1';

/**
 * THE ACCOUNT COHORT-14 EXISTS TO BE ABLE TO PAY, read by the browser.
 *
 * On 2026-09-03 cohort-14's market B `DUVcCGfj…` resolved, went Terminal, and
 * paid 500,000,000 atoms into `DsQSGKPb…` — an account created by the
 * Associated Token Account program with `spl-token create-account
 * --program-2022`, which is to say the account an ORDINARY WALLET already has.
 * Under Token-2022 the ATA program always appends `ImmutableOwner`, so it is
 * 170 bytes, and until this lane every TypeScript reader in this tree required
 * exactly 165 and refused it. The chain admitted it; the browser could not read
 * it.
 *
 * This is the live case for the repair. It reads two accounts and makes one
 * claim about each:
 *
 *   * the paid destination is 170 bytes, carries the exact empty
 *     `ImmutableOwner` entry, and is the canonical associated token account of
 *     the owner and mint written in its OWN bytes — so "the conventional
 *     destination" is proved by derivation rather than asserted;
 *   * the Hoard the payout debited is a base 165-byte account, admitted by the
 *     SAME function. One admission, two widths, and the pair is what stops this
 *     from being a test that would pass on a function that admits anything.
 *
 * Nothing here pins a balance or a phase: cohort-14 is devnet and this test
 * outlives it. What it pins is the WIDTH and the SUFFIX, which are properties
 * of the ATA program rather than of this cohort.
 *
 * Supply `DCLUTCH_LIVE_DEVNET=1`, and `DCLUTCH_LIVE_ENDPOINT` for a keyed
 * endpoint. Overridable per cohort with `DCLUTCH_PAID_DESTINATION` and
 * `DCLUTCH_PAID_HOARD`.
 *
 * Devnet evidence. Not mainnet evidence.
 */
const PAID_DESTINATION_V1 = process.env.DCLUTCH_PAID_DESTINATION ?? 'DsQSGKPbmJcZ89xts1Jgs1P5fprmX64fomqGFsQM1kmU';
const PAID_HOARD_V1 = process.env.DCLUTCH_PAID_HOARD ?? 'BrLJBohX4W6sLe3N9z21KqRuiDKdyG7XWnRuv4sVNQFr';
const ASSOCIATED_TOKEN_PROGRAM_V1 = 'ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL';

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' ? it : it.skip;

function client(): SolanaRpcClient {
  return new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? DEVNET_DEPLOYMENT_V1.endpoint);
}

describe('live devnet: the payout destination an ordinary wallet already has', () => {
  live('admits the paid 170-byte ATA and proves it is the owner’s own associated account', async () => {
    const observation = await client().accountInfo(PAID_DESTINATION_V1);
    expect(observation.account, `no account at ${PAID_DESTINATION_V1}`).not.toBeNull();
    const account = observation.account!;
    expect(account.owner).toBe(TOKEN_2022_PROGRAM_ID);
    expect(account.executable).toBe(false);

    // The finding, in one number and five bytes.
    expect(account.data).toHaveLength(TOKEN_ACCOUNT_IMMUTABLE_OWNER_BYTES_V1);
    expect(Array.from(account.data.subarray(TOKEN_ACCOUNT_BYTES_V1)))
      .toEqual(Array.from(TOKEN_ACCOUNT_IMMUTABLE_OWNER_SUFFIX_V1));

    const base = admitBaseOrImmutableOwnerTokenAccountV1(account.data, 'paid destination');
    expect(base).toHaveLength(TOKEN_ACCOUNT_BYTES_V1);

    // The browser's own reader, which refused this account outright before the
    // admission was shared with the chain's.
    const view = decodeToken2022BehaviorAccountV2(PAID_DESTINATION_V1, account);
    expect(view.address).toBe(PAID_DESTINATION_V1);

    // "The conventional destination", derived rather than asserted: the mint
    // and owner in the account's own bytes, run through the ATA program's
    // canonical seeds, return this address.
    const [derived] = PublicKey.findProgramAddressSync([
      new PublicKey(view.owner).toBytes(),
      new PublicKey(TOKEN_2022_PROGRAM_ID).toBytes(),
      new PublicKey(view.mint).toBytes(),
    ], new PublicKey(ASSOCIATED_TOKEN_PROGRAM_V1));
    expect(derived.toBase58()).toBe(PAID_DESTINATION_V1);
  });

  live('admits the base 165-byte Hoard through the same function', async () => {
    // The positive control. A function that returned its input unexamined would
    // pass the case above; this one fails unless the base width is genuinely a
    // second admitted shape rather than the only one.
    const observation = await client().accountInfo(PAID_HOARD_V1);
    expect(observation.account, `no account at ${PAID_HOARD_V1}`).not.toBeNull();
    const account = observation.account!;
    expect(account.owner).toBe(TOKEN_2022_PROGRAM_ID);
    expect(account.data).toHaveLength(TOKEN_ACCOUNT_BYTES_V1);
    expect(admitBaseOrImmutableOwnerTokenAccountV1(account.data, 'Hoard')).toBe(account.data);
    expect(decodeToken2022BehaviorAccountV2(PAID_HOARD_V1, account).mint)
      .toBe(decodeToken2022BehaviorAccountV2(PAID_DESTINATION_V1, (await client().accountInfo(PAID_DESTINATION_V1)).account!).mint);
  });
});
