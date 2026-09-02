import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import { SolanaRpcClient } from './rpc';
import { walletTerminalInputRequestJsonV1 } from './walletTerminalInputSnapshot';
import {
  loadWalletTerminalInputWasmV1,
  parseWalletTerminalInputAddressesV1,
} from './walletTerminalInputV1';

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' ? it : it.skip;

/**
 * PHASE ZERO against the chain it was written for.
 *
 * The address book the CLI projects out of a sealed campaign report, derived
 * instead from devnet's own accounts — and compared, row for row, with what
 * that report recorded for the same market. The four `terminal_composition_*`
 * rows are the ones worth watching: nothing on chain points at them, so they
 * are RECOMPILED here by the same function the founding published them with,
 * and matching the report is the whole claim.
 *
 * Gated on `DCLUTCH_LIVE_DEVNET=1` because it performs real network IO.
 * `DCLUTCH_LIVE_ENDPOINT` overrides the endpoint so a paid key can be supplied
 * from the environment and never written down here.
 *
 * The expected values below are devnet cohort-11 facts, copied from the
 * campaign report at `market/campaign-open.json` for market
 * `3rBfDBpa…` — the market that report names as its `founding_market`.
 */
const COHORT_11 = Object.freeze({
  endpoint: process.env.DCLUTCH_LIVE_ENDPOINT ?? 'https://api.devnet.solana.com',
  market: '3rBfDBpaXjKSbUU5HRaRTr6yhDQq4S1oKp2mQRsdoyb6',
  owner: 'BmDp2LRfAUxPw6qhQr9ceGMoitMtkQf3H547iTS631rv',
  programs: Object.freeze({
    registry: 'ADB72ar6ZSstXEg76Q1bPb5UY2EGmH6mrVfwr8K2fzom',
    core: 'FinXxc9drpmCYA7Cy4aGWSa1jYY87K6pNPfY9qFWzJCF',
    claims: 'HQYqqdzn5s6tEM6ywgeCr7Bd56tEuhpoop3ruvHRfAq6',
    custody: 'Cdh8Vv7DRyk7rhLcee574potYfaiVEsYR5HUPCrNPzCB',
    resolution: '3WqTxq6uKMK2d9f6uRujh8hCZvVB78KjGo9AYxvPQNVM',
  }),
  book: Object.freeze({
    collateralMint: 'H5zmg8nVY9JPccjYeB1t4d7AuLPJhVpjDnZMD718gGFk',
    tokenProgram: 'TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb',
    realm: 'CodzELyn3n8AzRjxUzaEU4dSDf3QqL6UjnWJxmaHcuVD',
    product: 'AXN3ZPNgxejboHCVLy8giVyDicodU7TLWwEjRn71qJxA',
    resultDomain: 'H9x2woSjNvVGizkuYP86GJhBdCZ3JNnTPBPfxt6QXDhZ',
    portfolio: 'CBCvDmv8GJWoD9H9G4AgQh8jY1QKMqdPqTd43tCbfATz',
    productBasis: 'HprHEBnudyLmbJkUSQ8US7B7tAvZ42Sc7XY7QCbTab9v',
    compositionDescriptor: 'EaLCCw8fSL8ZasmzfohLsUz1iWk7EpTUHGnnt3mX8btS',
    compositionGraph: '54HfTfUwN5Tjm5DTqLniXDSmsvhNwWfzRKdvPZqkVaF4',
    compositionTranslation: '1ZgJtz7Ry4XBoYwb24KJpeVpKL6TMVtpP8SYqrXQKcs',
    compositionExposure: '7sQdYzMpVmAtTVWy5s1LBfrLgAEJ2BGPyLa6A3f85MWh',
  }),
});

const SNAPSHOT_FORMAT = 'dclutch-wallet-terminal-payout-input-snapshot-v1';

function base64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

describe('live devnet payout-input address book', () => {
  live('derives all eleven rows from chain, and they are the campaign report’s', async () => {
    const client = new SolanaRpcClient(COHORT_11.endpoint);
    const derivation = await loadWalletTerminalInputWasmV1(async () => new Response(
      readFileSync(new URL('./generated/walletTerminalInputWasm/wallet_terminal_input_bg.wasm', import.meta.url)),
    ));
    const request = walletTerminalInputRequestJsonV1({
      programs: COHORT_11.programs,
      market: COHORT_11.market,
      owner: COHORT_11.owner,
      // Unused by phase zero: the destination only enters the payout frame.
      recipient: COHORT_11.owner,
      claimIndex: 0,
    });

    // ONE floor for all three rounds.
    const floor = await client.finalizedSlot();
    const unixTimestamp = (await client.blockTime(floor)) ?? '0';
    const round = async (addressesJson: string) => {
      const addresses = parseWalletTerminalInputAddressesV1(addressesJson);
      const observed = await client.multipleAccounts([...addresses], floor);
      return JSON.stringify({
        format: SNAPSHOT_FORMAT,
        slot: floor,
        unixTimestamp,
        keys: [...addresses],
        accounts: observed.accounts.map((entry) => entry.account === null ? null : ({
          key: entry.address,
          owner: entry.account.owner,
          lamports: entry.account.lamports,
          executable: entry.account.executable,
          dataBase64: base64(entry.account.data),
        })),
      });
    };

    const one = await round(derivation.wallet_terminal_input_round_one_addresses_v1(request));
    const two = await round(derivation.wallet_terminal_input_book_round_two_addresses_v1(request, one));
    const three = await round(derivation.wallet_terminal_input_book_round_three_addresses_v1(request, one, two));
    const derived = JSON.parse(derivation.derive_wallet_terminal_input_request_v1(request, one, two, three));

    const records = derived.routing.records;
    expect(derived.routing.foundingMarket).toBe(COHORT_11.market);
    expect(derived.routing.collateralMint).toBe(COHORT_11.book.collateralMint);
    expect(derived.routing.tokenProgram).toBe(COHORT_11.book.tokenProgram);
    for (const row of ['realm', 'product', 'resultDomain', 'portfolio', 'productBasis',
      'compositionDescriptor', 'compositionGraph', 'compositionTranslation', 'compositionExposure'] as const) {
      expect(records[row].address, row).toBe(COHORT_11.book[row]);
      expect(records[row].digest, `${row} digest`).toMatch(/^[0-9a-f]{64}$/);
    }
  }, 120_000);

  live('refuses a market whose Claims aggregate was never created', async () => {
    // `ARuPAu…` is a Core account on the same cohort with no Claims aggregate
    // at any finalized floor. A redemption cannot begin there, and phase zero
    // says so at the account rather than forty accounts later.
    const client = new SolanaRpcClient(COHORT_11.endpoint);
    const derivation = await loadWalletTerminalInputWasmV1(async () => new Response(
      readFileSync(new URL('./generated/walletTerminalInputWasm/wallet_terminal_input_bg.wasm', import.meta.url)),
    ));
    const request = walletTerminalInputRequestJsonV1({
      programs: COHORT_11.programs,
      market: 'ARuPAuyJbJoLdMWGDzSqvcV9py25EkmMj8ABnfKP56s',
      owner: COHORT_11.owner,
      recipient: COHORT_11.owner,
      claimIndex: 0,
    });
    const floor = await client.finalizedSlot();
    const unixTimestamp = (await client.blockTime(floor)) ?? '0';
    const addresses = parseWalletTerminalInputAddressesV1(
      derivation.wallet_terminal_input_round_one_addresses_v1(request),
    );
    const observed = await client.multipleAccounts([...addresses], floor);
    const one = JSON.stringify({
      format: SNAPSHOT_FORMAT, slot: floor, unixTimestamp, keys: [...addresses],
      accounts: observed.accounts.map((entry) => entry.account === null ? null : ({
        key: entry.address, owner: entry.account.owner, lamports: entry.account.lamports,
        executable: entry.account.executable, dataBase64: base64(entry.account.data),
      })),
    });
    expect(() => derivation.wallet_terminal_input_book_round_two_addresses_v1(request, one))
      .toThrow(/Claims aggregate/);
  }, 120_000);
});
