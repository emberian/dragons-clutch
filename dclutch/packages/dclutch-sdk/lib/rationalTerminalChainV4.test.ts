import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  CORE_PHASE_TERMINAL_TAG,
  CORE_READINESS_CONSUMED_TAG,
  CORE_STATE_BYTES,
  CORE_STATE_CAPABILITY_MANIFEST_OFFSET,
  CORE_STATE_GENERATION_OFFSET,
  CORE_STATE_IDENTITY_REALM_OFFSET,
  CORE_STATE_MAGIC,
  CORE_STATE_MARKET_ID_OFFSET,
  CORE_STATE_PRINCIPAL_CAP_SETS_OFFSET,
  CORE_STATE_PRODUCT_ID_OFFSET,
  CORE_STATE_PRODUCT_RECORD_OFFSET,
  CORE_STATE_READINESS_OFFSET,
  CORE_STATE_REGISTRY_PROGRAM_OFFSET,
  CORE_STATE_RENT_BENEFICIARY_OFFSET,
  CORE_STATE_RESOLUTION_POLICY_OFFSET,
  CORE_STATE_SELECTED_RELEASE_SET_OFFSET,
  CORE_STATE_TERMINAL_RECEIPT_OFFSET,
  CORE_STATE_TERMINAL_WINNER_OFFSET,
  CORE_STATE_VERSION_OFFSET,
  CORE_STATE_PHASE_OFFSET,
  CORE_VERSION,
} from './generated/coreFound';
import { RESOLUTION_CERTIFICATE_V2_WIDE_SUCCESS_EXAMPLE_HEX } from './generated/resolutionCertificateV2';
import {
  authenticateCoreTerminalResolutionCertificateV4,
} from './rationalTerminalChainV4';
import {
  authenticateRationalHotCoreV3,
  type RationalHotRpcV4,
} from './rationalRetireReceiptV4';
import { type RpcAccount } from './rpc';

function identity(tag: number): Uint8Array {
  const output = new Uint8Array(32); output[0] = tag; return output;
}

function address(tag: number): string { return new PublicKey(identity(tag)).toBase58(); }

function hexBytes(value: string): Uint8Array {
  return Uint8Array.from(value.match(/../g) ?? [], (byte) => Number.parseInt(byte, 16));
}

function currentTerminalCore(): Uint8Array {
  const output = new Uint8Array(CORE_STATE_BYTES);
  const view = new DataView(output.buffer);
  output.set(CORE_STATE_MAGIC, 0);
  view.setUint16(CORE_STATE_VERSION_OFFSET, CORE_VERSION, true);
  output[CORE_STATE_PHASE_OFFSET] = CORE_PHASE_TERMINAL_TAG;
  output[CORE_STATE_READINESS_OFFSET] = CORE_READINESS_CONSUMED_TAG;
  view.setUint32(CORE_STATE_TERMINAL_WINNER_OFFSET, 257, true);
  for (const [offset, tag] of [
    [CORE_STATE_MARKET_ID_OFFSET, 1],
    [CORE_STATE_IDENTITY_REALM_OFFSET, 2],
    [CORE_STATE_PRODUCT_RECORD_OFFSET, 4],
    [CORE_STATE_PRODUCT_ID_OFFSET, 5],
    [CORE_STATE_RESOLUTION_POLICY_OFFSET, 3],
    [CORE_STATE_CAPABILITY_MANIFEST_OFFSET, 7],
    [CORE_STATE_SELECTED_RELEASE_SET_OFFSET, 8],
    [CORE_STATE_REGISTRY_PROGRAM_OFFSET, 9],
    [CORE_STATE_RENT_BENEFICIARY_OFFSET, 10],
    [CORE_STATE_TERMINAL_RECEIPT_OFFSET, 6],
  ] as const) output.set(identity(tag), offset);
  view.setBigUint64(CORE_STATE_GENERATION_OFFSET, 9n, true);
  view.setBigUint64(CORE_STATE_PRINCIPAL_CAP_SETS_OFFSET, 123n, true);
  return output;
}

function account(data: Uint8Array, owner: string, lamports = '100'): RpcAccount {
  return Object.freeze({ data, owner, lamports, executable: false, space: data.length });
}

function rpc(certificate: RpcAccount): RationalHotRpcV4 {
  return Object.freeze({
    async finalizedSlot() { return '40'; },
    async multipleAccounts(addresses: ReadonlyArray<string>) {
      return Object.freeze({ slot: '41', accounts: addresses.map((entry) => Object.freeze({ address: entry, account: certificate })) });
    },
    async minimumBalanceForRentExemption(dataLength: number) {
      return Object.freeze({ dataLength, lamports: '100' });
    },
  });
}

describe('current Core to terminal ResolutionCertificateV2 chain', () => {
  it('joins DCLTCOR3 offsets, activated owner, exact rent, and full-width certificate facts', async () => {
    const coreProgram = address(30); const resolutionProgram = address(31);
    const market = authenticateRationalHotCoreV3(address(1), account(currentTerminalCore(), coreProgram), coreProgram);
    expect(market).toMatchObject({
      phase: 'Terminal', readiness: 'Consumed', terminalWinner: 257,
      generation: 9n, principalCapSets: 123n, rentBeneficiary: address(10),
    });
    expect(market.terminalReceipt).toEqual(identity(6));
    const joined = await authenticateCoreTerminalResolutionCertificateV4(
      rpc(account(hexBytes(RESOLUTION_CERTIFICATE_V2_WIDE_SUCCESS_EXAMPLE_HEX), resolutionProgram)),
      { observedSlot: '40', marketAddress: address(1), market, resolutionProgram, outcomeCount: 259 },
    );
    expect(joined).toMatchObject({ observedSlot: '41', address: address(6) });
    expect(joined.certificate).toMatchObject({
      kind: 'resolution-success', selector: 257, resultNumerator: 7n, resultDenominator: 1n,
    });
  });

  it('explicitly refuses historical DCLTCOR2 before any certificate read', () => {
    const historical = currentTerminalCore().slice(0, 352);
    historical.set(new TextEncoder().encode('DCLTCOR2'), 0);
    new DataView(historical.buffer).setUint16(CORE_STATE_VERSION_OFFSET, 2, true);
    expect(() => authenticateRationalHotCoreV3(address(1), account(historical, address(30)), address(30)))
      .toThrow('This older devnet Market generation is incompatible');
  });

  it('refuses a substituted Resolution owner and a below-rent certificate', async () => {
    const coreProgram = address(30); const resolutionProgram = address(31);
    const market = authenticateRationalHotCoreV3(address(1), account(currentTerminalCore(), coreProgram), coreProgram);
    const certificate = hexBytes(RESOLUTION_CERTIFICATE_V2_WIDE_SUCCESS_EXAMPLE_HEX);
    await expect(authenticateCoreTerminalResolutionCertificateV4(
      rpc(account(certificate, address(32))),
      { observedSlot: '40', marketAddress: address(1), market, resolutionProgram, outcomeCount: 259 },
    )).rejects.toThrow('Resolution-owned');
    await expect(authenticateCoreTerminalResolutionCertificateV4(
      rpc(account(certificate, resolutionProgram, '99')),
      { observedSlot: '40', marketAddress: address(1), market, resolutionProgram, outcomeCount: 259 },
    )).rejects.toThrow('below its current exact rent minimum');
  });
});
