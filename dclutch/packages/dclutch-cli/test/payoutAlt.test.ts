import { createHash } from 'node:crypto';

import {
  AddressLookupTableProgram,
  Keypair,
  PublicKey,
  Transaction,
  type TransactionInstruction,
} from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  nextWalletTerminalPayoutAltActionV1,
  parseWalletTerminalPayoutAltPlanV1,
  provisionWalletTerminalPayoutAltV1,
  type InstructionManifestV1,
  type WalletTerminalPayoutAltObservationV1,
} from '../src/payoutAlt';

function key(byte: number): string {
  return new PublicKey(new Uint8Array(32).fill(byte)).toBase58();
}

function instruction(value: TransactionInstruction): InstructionManifestV1 {
  return Object.freeze({
    programId: value.programId.toBase58(),
    accounts: Object.freeze(value.keys.map((meta) => Object.freeze({
      address: meta.pubkey.toBase58(),
      signer: meta.isSigner,
      writable: meta.isWritable,
    }))),
    dataBase64: Buffer.from(value.data).toString('base64'),
  });
}

function fixture(owner = key(2), width = 25) {
  const source = {
    format: 'dclutch-wallet-terminal-payout-plan-input-v1',
    market: key(1), owner, recipientOwner: owner, recipient: key(3), collateralMint: key(4),
    tokenProgram: key(5), quantity: '7', claimIndex: 1, transferIndex: 0,
    parentContext: '06'.repeat(32), custodyContext: '07'.repeat(32), releaseSet: '08'.repeat(32),
    terminalCertificate: key(30),
    programs: { registry: key(10), core: key(11), claims: key(12), custody: key(13), resolution: key(14) },
    records: {
      realm: '14'.repeat(32), product: '15'.repeat(32), resultDomain: '16'.repeat(32),
      portfolio: '17'.repeat(32), productBasis: '18'.repeat(32), compositionDescriptor: '1a'.repeat(32),
      compositionGraph: '1b'.repeat(32), compositionTranslation: '1c'.repeat(32),
      compositionExposure: '1d'.repeat(32),
    },
  };
  const sourceBytes = Buffer.from(JSON.stringify(source));
  const addresses = Array.from({ length: width }, (_, index) => key(index % 2 === 0 ? 80 - index : 20 + index));
  const observationSlot = 44;
  const [create, lookupTable] = AddressLookupTableProgram.createLookupTable({
    authority: new PublicKey(owner), payer: new PublicKey(owner), recentSlot: observationSlot,
  });
  const extensions: InstructionManifestV1[] = [];
  for (let offset = 0; offset < addresses.length; offset += 20) {
    extensions.push(instruction(AddressLookupTableProgram.extendLookupTable({
      lookupTable,
      authority: new PublicKey(owner),
      payer: new PublicKey(owner),
      addresses: addresses.slice(offset, offset + 20).map((address) => new PublicKey(address)),
    })));
  }
  const value = {
    format: 'dclutch-wallet-terminal-payout-alt-plan-v1',
    sourceInputSha256: createHash('sha256').update(sourceBytes).digest('hex'),
    observationSlot: String(observationSlot),
    payer: owner,
    authority: owner,
    lookupTable: lookupTable.toBase58(),
    addresses,
    create: instruction(create),
    extensions,
    payoutInput: { ...source, lookupTable: lookupTable.toBase58() },
  };
  return { sourceBytes, value, plan: parseWalletTerminalPayoutAltPlanV1(JSON.stringify(value), sourceBytes) };
}

function observation(
  plan: ReturnType<typeof fixture>['plan'],
  addresses: ReadonlyArray<string>,
  slot = '100',
  lastExtendedSlot = '99',
): WalletTerminalPayoutAltObservationV1 {
  return Object.freeze({
    slot,
    owner: AddressLookupTableProgram.programId.toBase58(),
    executable: false,
    authority: plan.authority,
    deactivationSlot: '18446744073709551615',
    lastExtendedSlot,
    addresses: Object.freeze([...addresses]),
  });
}

describe('wallet payout ordered ALT handoff', () => {
  it('hostile-parses the Rust plan and independently pins every official instruction', () => {
    const { plan, sourceBytes, value } = fixture();
    expect(plan.addresses).toEqual(value.addresses);
    expect(plan.addresses).not.toEqual([...plan.addresses].sort());

    expect(() => parseWalletTerminalPayoutAltPlanV1(JSON.stringify({ ...value, extra: true }), sourceBytes)).toThrow(/missing or unknown fields/);
    expect(() => parseWalletTerminalPayoutAltPlanV1(JSON.stringify(value), Buffer.from('{}'))).toThrow(/another source input/);
    expect(() => parseWalletTerminalPayoutAltPlanV1(JSON.stringify({
      ...value,
      addresses: [...value.addresses].reverse(),
    }), sourceBytes)).toThrow(/exact official ALT instruction/);
    expect(() => parseWalletTerminalPayoutAltPlanV1(JSON.stringify({
      ...value,
      create: { ...value.create, dataBase64: 'AA==' },
    }), sourceBytes)).toThrow(/exact official ALT instruction/);
  });

  it('resumes only exact complete finalized prefixes and waits one slot after the last extension', () => {
    const { plan } = fixture();
    const absent: WalletTerminalPayoutAltObservationV1 = Object.freeze({
      slot: '90', owner: null, executable: false, authority: null,
      deactivationSlot: '18446744073709551615', lastExtendedSlot: '0', addresses: Object.freeze([]),
    });
    expect(nextWalletTerminalPayoutAltActionV1(plan, absent)).toEqual({ kind: 'create' });
    expect(() => nextWalletTerminalPayoutAltActionV1(plan, { ...absent, slot: '556' })).toThrow(/expired from SlotHashes/);
    expect(nextWalletTerminalPayoutAltActionV1(plan, observation(plan, []))).toEqual({ kind: 'extend', page: 0 });
    expect(nextWalletTerminalPayoutAltActionV1(plan, observation(plan, plan.addresses.slice(0, 20)))).toEqual({ kind: 'extend', page: 1 });
    expect(nextWalletTerminalPayoutAltActionV1(plan, observation(plan, plan.addresses, '99', '99'))).toEqual({ kind: 'wait', minimumSlot: '100' });
    expect(nextWalletTerminalPayoutAltActionV1(plan, observation(plan, plan.addresses, '100', '99'))).toEqual({ kind: 'ready', finalizedSlot: '100' });

    expect(() => nextWalletTerminalPayoutAltActionV1(plan, observation(plan, plan.addresses.slice(0, 7)))).toThrow(/complete planned extension page/);
    expect(() => nextWalletTerminalPayoutAltActionV1(plan, observation(plan, [key(99), ...plan.addresses.slice(1, 20)]))).toThrow(/exact prefix/);
    expect(() => nextWalletTerminalPayoutAltActionV1(plan, {
      ...observation(plan, []), authority: key(88),
    })).toThrow(/another authority/);
  });

  it('signs create and each missing page once, then requires a later finalized readback', async () => {
    const signer = Keypair.generate();
    const { plan } = fixture(signer.publicKey.toBase58());
    const states: WalletTerminalPayoutAltObservationV1[] = [
      Object.freeze({
        slot: '90', owner: null, executable: false, authority: null,
        deactivationSlot: '18446744073709551615', lastExtendedSlot: '0', addresses: Object.freeze([]),
      }),
      Object.freeze({
        slot: '90', owner: null, executable: false, authority: null,
        deactivationSlot: '18446744073709551615', lastExtendedSlot: '0', addresses: Object.freeze([]),
      }),
      observation(plan, [], '91', '0'),
      observation(plan, [], '91', '0'),
      observation(plan, plan.addresses.slice(0, 20), '92', '92'),
      observation(plan, plan.addresses.slice(0, 20), '92', '92'),
      observation(plan, plan.addresses, '93', '93'),
      observation(plan, plan.addresses, '93', '93'),
      observation(plan, plan.addresses, '94', '93'),
    ];
    const wires: Uint8Array[] = [];
    let reads = 0;
    const result = await provisionWalletTerminalPayoutAltV1(plan, signer, {
      observe: async () => states[Math.min(reads++, states.length - 1)]!,
      latestMutationBlockhash: async () => ({ blockhash: key(90) }),
      submit: async (wire) => { wires.push(wire); return true; },
      wait: async () => {},
    });
    expect(result).toEqual({ transactions: 3, finalizedSlot: '94' });
    expect(wires).toHaveLength(3);
    for (const wire of wires) expect(Transaction.from(wire).verifySignatures()).toBe(true);

    await expect(provisionWalletTerminalPayoutAltV1(plan, Keypair.generate(), {
      observe: async () => states[0]!,
      latestMutationBlockhash: async () => ({ blockhash: key(90) }),
      submit: async () => true,
      wait: async () => {},
    })).rejects.toThrow(/explicit signer/);
  });
});
