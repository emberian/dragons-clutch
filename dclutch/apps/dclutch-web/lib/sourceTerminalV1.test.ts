import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { SOURCE_TERMINAL_PLAN_FORMAT_V1 } from '@dclutch/sdk/generated/sourceReadinessWasmV1';
import { buildSourceTerminalTransactionV1, parseSourceTerminalPlanV1 } from './sourceTerminalV1';

const address = (byte: number) => new PublicKey(new Uint8Array(32).fill(byte)).toBase58();

function planValue(): Record<string, unknown> {
  const metas = Array.from({ length: 22 }, (_, index) => ({
    address: address(index + 2), isSigner: false, isWritable: [1, 12, 13].includes(index),
  }));
  return {
    format: SOURCE_TERMINAL_PLAN_FORMAT_V1, route: 'admit', observedSlot: '7',
    instruction: { program: address(30), accounts: metas, dataBase64: 'AQ==' }, prepay: null,
    accounts: { protocolWritable: [metas[1]!.address, metas[12]!.address, metas[13]!.address],
      completion: [metas[1]!.address, metas[12]!.address, metas[13]!.address] },
    geometry: { protocolAccountCount: 22, protocolUniqueAccountCount: 23, protocolWritableCount: 3,
      protocolSignerCount: 0, protocolDataLen: 1, transactionInstructionCountWithoutComputeBudget: 1,
      transactionLockCountWithoutPayer: 23 },
    facts: { terminal: 'false', selector: '1', outcomeCount: '3' },
  };
}

describe('Source terminal browser transport', () => {
  it('compiles the exact signer-free Rust plan with the wallet as sole payer', () => {
    const source = JSON.stringify(planValue());
    const plan = parseSourceTerminalPlanV1(source);
    const result = buildSourceTerminalTransactionV1({ plan, planJson: source, snapshotJson: '{}', observationAddresses: Object.freeze([]) },
      address(31), { blockhash: address(32), lastValidBlockHeight: '99' });
    expect(result.transaction.signatures).toHaveLength(1);
    expect(result.transaction.message.header.numRequiredSignatures).toBe(1);
    expect(result.payer).toBe(address(31));
  });

  it('refuses unknown fields, another signer, or changed 22-account geometry', () => {
    const unknown = planValue(); unknown.extra = true;
    expect(() => parseSourceTerminalPlanV1(JSON.stringify(unknown))).toThrow(/unknown fields/);
    const signer = planValue();
    ((signer.instruction as { accounts: Array<{ isSigner: boolean }> }).accounts[4]!).isSigner = true;
    expect(() => parseSourceTerminalPlanV1(JSON.stringify(signer))).toThrow(/changed authority/);
    const short = planValue();
    (short.instruction as { accounts: unknown[] }).accounts.pop();
    expect(() => parseSourceTerminalPlanV1(JSON.stringify(short))).toThrow(/22-account frame/);
  });
});
