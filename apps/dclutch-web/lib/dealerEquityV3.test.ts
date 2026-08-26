import {
  AddressLookupTableAccount,
  PublicKey,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  SYSVAR_RENT_PUBKEY,
} from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  type DealerEquityHotRouteV3,
  compileDealerEquityTransactionV3,
  decodeDealerEquityRequestV3,
} from './dealerEquityV3';
import {
  HOT_FIXED_ACCOUNT_COUNT_V3,
  HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
  HOT_MARKET_ACCOUNT_V3,
  HOT_RENT_SYSVAR_ACCOUNT_V3,
  HOT_ROOT_ACCOUNT_V3,
  HOT_TRADING_PROGRAM_ACCOUNT_V3,
} from './generated/directInlineV3';
import {
  DEALER_EQUITY_HEADER_BYTES_V3,
  DEALER_LP_POSITION_PDA_DOMAIN_V3,
  DEALER_OBLIGATION_PDA_DOMAIN_V3,
} from './generated/dealerEquityV3';
import { type DirectHotAccountMetaV3 } from './directInlineV3';

const MAX_U64 = 18_446_744_073_709_551_615n;

function address(seed: number): string {
  const bytes = new Uint8Array(32);
  new DataView(bytes.buffer).setUint32(0, seed, true);
  bytes[31] = 1;
  return new PublicKey(bytes).toBase58();
}

function meta(value: string, writable = false, executable = false): DirectHotAccountMetaV3 {
  return Object.freeze({ address: value, isSigner: false, isWritable: writable, executable });
}

function put64(output: Uint8Array, offset: number, value: bigint): void {
  new DataView(output.buffer, output.byteOffset + offset, 8).setBigUint64(0, value, true);
}

function fixture(): Readonly<{ route: DealerEquityHotRouteV3; request: Uint8Array }> {
  const trading = address(2);
  const market = address(3);
  const root = address(4);
  const lpOwner = address(5);
  const release = new Uint8Array(32).fill(6);
  const obligation = PublicKey.findProgramAddressSync([DEALER_OBLIGATION_PDA_DOMAIN_V3, new PublicKey(root).toBytes()], new PublicKey(trading))[0];
  const lp = PublicKey.findProgramAddressSync([DEALER_LP_POSITION_PDA_DOMAIN_V3, new PublicKey(root).toBytes(), new PublicKey(lpOwner).toBytes()], new PublicKey(trading))[0];
  const request = new Uint8Array(DEALER_EQUITY_HEADER_BYTES_V3);
  request.set(new TextEncoder().encode('DCLMEQ03'), 0);
  new DataView(request.buffer).setUint16(8, 2, true);
  new DataView(request.buffer).setUint16(10, 1, true);
  new DataView(request.buffer).setUint32(12, 3, true);
  for (const [offset, value] of [
    [16, release], [48, new PublicKey(market).toBytes()], [80, new PublicKey(root).toBytes()],
    [112, lp.toBytes()], [144, new PublicKey(lpOwner).toBytes()], [176, obligation.toBytes()],
    [208, new Uint8Array(32).fill(10)], [240, new Uint8Array(32).fill(11)], [272, new Uint8Array(32).fill(12)],
    [304, new Uint8Array(32).fill(13)], [336, new Uint8Array(32).fill(14)], [368, new Uint8Array(32).fill(15)],
  ] as const) request.set(value, offset);
  for (const [offset, value] of [[400, 1n], [408, 2n], [416, 3n], [424, 4n], [432, 7n], [440, 1_100n], [448, 5n], [456, 25n], [464, 10n]] as const) put64(request, offset, value);

  const fixed = Array.from({ length: HOT_FIXED_ACCOUNT_COUNT_V3 }, (_, index) => meta(address(100 + index)));
  fixed[HOT_MARKET_ACCOUNT_V3] = meta(market);
  fixed[HOT_ROOT_ACCOUNT_V3] = meta(root, true);
  fixed[HOT_TRADING_PROGRAM_ACCOUNT_V3] = meta(trading, false, true);
  fixed[HOT_RENT_SYSVAR_ACCOUNT_V3] = meta(SYSVAR_RENT_PUBKEY.toBase58());
  fixed[HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3] = meta(SYSVAR_INSTRUCTIONS_PUBKEY.toBase58());
  const strategy = Array.from({ length: 10 }, (_, index) => meta(address(200 + index), false, index === 6));
  const runtime = Array.from({ length: 50 }, (_, index) => meta(address(300 + index), index === 48 || index === 49));
  const addresses = [...fixed, ...strategy, ...runtime].map((entry) => new PublicKey(entry.address));
  const table = new AddressLookupTableAccount({
    key: new PublicKey(address(900)),
    state: { deactivationSlot: MAX_U64, lastExtendedSlot: 800, lastExtendedSlotStartIndex: 0, authority: undefined, addresses },
  });
  return Object.freeze({
    request,
    route: Object.freeze({
      payer: address(901), tradingProgram: trading, market, releaseSet: release, generation: 7n,
      rootPrestateDigest: new Uint8Array(32).fill(17), observedSlot: 1_000n,
      fixedAccounts: Object.freeze(fixed), strategyAccounts: Object.freeze(strategy), runtimeAccounts: Object.freeze(runtime),
      recentBlockhash: address(902), lookupTables: Object.freeze([table]),
      outerEvidence: Object.freeze({ status: 'checked' as const, tradingArtifactRelease: '18'.repeat(32), checkedManifestDigest: '19'.repeat(32) }),
    }),
  });
}

describe('Dealer V3 equity transaction construction', () => {
  it('hostile-decodes P0 contribution and compiles one bounded unsigned Hot transaction', async () => {
    const value = fixture();
    const decoded = await decodeDealerEquityRequestV3(value.request);
    expect(decoded.action).toBe('contribute');
    expect(decoded.signedPositionCount).toBe(0);
    expect(decoded.width).toBe(3);
    const plan = await compileDealerEquityTransactionV3(value.route, value.request);
    expect(plan.request.lpPosition).toBe(decoded.lpPosition);
    expect(plan.transaction.message.compiledInstructions).toHaveLength(1);
    expect(plan.requiredSigners).toEqual([value.route.payer]);
    expect(plan.wireBytes.length).toBeLessThanOrEqual(1_232);
    expect(plan.loadedAddresses).toBeGreaterThan(80);
  });

  it('refuses stale expiry, substituted PDA, unrecognized release, and P0 suffix bytes', async () => {
    const value = fixture();
    await expect(compileDealerEquityTransactionV3({ ...value.route, observedSlot: 1_101n }, value.request)).rejects.toThrow(/expiry/);
    const hostile = new Uint8Array(value.request);
    hostile.set(new PublicKey(address(990)).toBytes(), 112);
    await expect(compileDealerEquityTransactionV3(value.route, hostile)).rejects.toThrow(/canonical Trading PDA/);
    await expect(compileDealerEquityTransactionV3({ ...value.route, outerEvidence: { status: 'unavailable', reason: 'not checked' } }, value.request)).rejects.toThrow(/not checked/);
    const suffix = new Uint8Array(value.request.length + 1); suffix.set(value.request); suffix[472] = 1; suffix[480] = 1;
    await expect(decodeDealerEquityRequestV3(suffix)).rejects.toThrow(/P0/);
  });
});
