import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { SOURCE_CLOSE_PLAN_FORMAT_V1 } from './generated/sourceReadinessWasmV1';
import { buildSourceCloseFundTransactionV1, parseSourceCloseFundPlanV1 } from './sourceCloseFundV1';

const address = (byte: number) => new PublicKey(new Uint8Array(32).fill(byte)).toBase58();

function closePlan(): Record<string, unknown> {
  const metas = Array.from({ length: 19 }, (_, index) => ({
    address: address(index + 2), isSigner: false, isWritable: [11, 12, 14, 15].includes(index),
  }));
  return {
    format: SOURCE_CLOSE_PLAN_FORMAT_V1, route: 'close', observedSlot: '7', prepay: null,
    instruction: { program: address(30), accounts: metas, dataBase64: 'AQ==' },
    accounts: { protocolWritable: [metas[11]!.address, metas[12]!.address, metas[14]!.address, metas[15]!.address],
      completion: [metas[11]!.address, metas[12]!.address, metas[14]!.address, metas[15]!.address] },
    geometry: { protocolAccountCount: 19, protocolUniqueAccountCount: 20, protocolWritableCount: 4,
      protocolSignerCount: 0, protocolDataLen: 1, transactionInstructionCountWithoutComputeBudget: 1,
      transactionLockCountWithoutPayer: 20 },
    facts: { closureReceipt: metas[14]!.address, requestDigest: '11'.repeat(32) },
  };
}

function prepayPlan(): Record<string, unknown> {
  return {
    format: SOURCE_CLOSE_PLAN_FORMAT_V1, route: 'prepay', observedSlot: '7', instruction: null,
    prepay: { destination: address(20), lamports: '99' }, accounts: null, geometry: null,
    facts: { currentLamports: '1', exactRentLamports: '100', receipt: address(20) },
  };
}

describe('Source close browser transport', () => {
  it('compiles exact prepay and direct close as sole-payer packets', () => {
    for (const value of [prepayPlan(), closePlan()]) {
      const source = JSON.stringify(value);
      const plan = parseSourceCloseFundPlanV1(source);
      const result = buildSourceCloseFundTransactionV1({ plan, planJson: source, snapshotJson: '{}', observationAddresses: [] },
        address(31), { blockhash: address(32), lastValidBlockHeight: '99' });
      expect(result.transaction.signatures).toHaveLength(1);
      expect(result.transaction.message.header.numRequiredSignatures).toBe(1);
      expect(result.payer).toBe(address(31));
      expect(result.route).toBe(plan.route);
    }
  });

  it('refuses route substitution, unknown fields, signers, and frame changes', () => {
    const unknown = closePlan(); unknown.extra = true;
    expect(() => parseSourceCloseFundPlanV1(JSON.stringify(unknown))).toThrow(/unknown fields/);
    const signer = closePlan();
    ((signer.instruction as { accounts: Array<{ isSigner: boolean }> }).accounts[3]!).isSigner = true;
    expect(() => parseSourceCloseFundPlanV1(JSON.stringify(signer))).toThrow(/changed authority/);
    const short = closePlan(); (short.instruction as { accounts: unknown[] }).accounts.pop();
    expect(() => parseSourceCloseFundPlanV1(JSON.stringify(short))).toThrow(/19\/21 account frame/);
    const mixed = prepayPlan(); mixed.instruction = (closePlan().instruction);
    expect(() => parseSourceCloseFundPlanV1(JSON.stringify(mixed))).toThrow(/route disagrees/);
  });
});
