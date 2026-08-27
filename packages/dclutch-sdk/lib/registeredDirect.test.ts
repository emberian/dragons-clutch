import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  REGISTERED_CONTROLLER_EXAMPLE,
  REGISTERED_CREATE_EXAMPLE,
  REGISTERED_RETIRE_EXAMPLE,
  REGISTERED_STATE_BYTES_VALUE,
  REGISTERED_STATE_EXAMPLE,
  REGISTERED_TERMINAL_CANCEL_EXAMPLE,
  REGISTERED_TERMINAL_EXPIRE_EXAMPLE,
} from './generated/registeredDirect';
import { decodeCompactIntentV1 } from './directCodec';
import {
  LEGACY_TOKEN_PROGRAM_ID,
  buildRegisteredCreateTransaction,
  buildRegisteredRetireTransaction,
  buildRegisteredFillTransaction,
  buildRegisteredTerminalTransaction,
  decodeRegisteredIntentStateV1,
  deriveRegisteredCreateAddresses,
  deriveRegisteredAddress,
  encodeRegisteredFillInstructionV1,
  encodeRegisteredCreateInstructionV1,
  encodeRegisteredRetireInstructionV1,
  encodeRegisteredIntentStateV1,
  encodeRegisteredTerminal,
  registeredBuyerReserve,
  registeredRetirementDelegation,
  scanRegisteredDirectStates,
  type RegisteredDirectStateObservation,
} from './registeredDirect';
import { CLAIM_PROGRAM_ID } from './directTransaction';
import { SolanaRpcClient } from './rpc';

function key(byte: number): PublicKey {
  return new PublicKey(new Uint8Array(32).fill(byte));
}

function base64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

function response(result: unknown): Response {
  return new Response(JSON.stringify({ jsonrpc: '2.0', id: 1, result }), { status: 200 });
}

function observed(controllerProgram: PublicKey, side: number, makerByte: number, collateralByte: number): RegisteredDirectStateObservation {
  const [controller] = PublicKey.findProgramAddressSync([new TextEncoder().encode('dclutch-controller-v1')], controllerProgram);
  const state = {
    phase: 0,
    controller: controller.toBase58(),
    maker: key(makerByte).toBase58(),
    intent: {
      side, outcome: 1, lifecycle: 2, market: key(4).toBytes(), generation: 3n, nonce: BigInt(makerByte),
      validFrom: 0n, validThrough: 100n, maximumFill: 2_000n,
      limitPrice: side === 0 ? 400_000n : 600_000n, feeBasisPoints: 25,
      collateralAccount: key(collateralByte).toBytes(),
    },
    remaining: 2_000n,
    sequence: 7n,
  };
  const derived = deriveRegisteredAddress(controllerProgram.toBase58(), state);
  return Object.freeze({ status: 'accepted', address: derived.address.toBase58(), observedSlot: '55', lamports: '1', bump: derived.bump, state: Object.freeze(state) });
}

describe('Lean-emitted registered Direct browser ABI', () => {
  it('strictly decodes and re-encodes the exact generated 232-byte example', () => {
    const state = decodeRegisteredIntentStateV1(REGISTERED_STATE_EXAMPLE);
    expect(encodeRegisteredIntentStateV1(state)).toEqual(REGISTERED_STATE_EXAMPLE);
    expect(state.remaining).toBe(2_000n);
    expect(state.sequence).toBe(0n);
    expect(state.intent.outcome).toBe(1);
  });

  it('matches exact generated fill, cancel, and expiry examples byte-for-byte', () => {
    expect(encodeRegisteredFillInstructionV1(2_000n, 500_000n, [1, 2, 3, 4, 5])).toEqual(REGISTERED_CONTROLLER_EXAMPLE);
    expect(encodeRegisteredTerminal('cancel', 2, 3, 7n)).toEqual(REGISTERED_TERMINAL_CANCEL_EXAMPLE);
    expect(encodeRegisteredTerminal('expire', 2, 3, 7n)).toEqual(REGISTERED_TERMINAL_EXPIRE_EXAMPLE);
  });

  it('matches the exact generated 152-byte creation example and derives gap-free addresses', () => {
    const intent = decodeCompactIntentV1(REGISTERED_CREATE_EXAMPLE.slice(16));
    expect(encodeRegisteredCreateInstructionV1(intent, 1, 2, 3)).toEqual(REGISTERED_CREATE_EXAMPLE);
    const first = deriveRegisteredCreateAddresses(key(67).toBase58(), key(4).toBase58(), 3n, key(5).toBase58(), 0n);
    const next = deriveRegisteredCreateAddresses(key(67).toBase58(), key(4).toBase58(), 3n, key(5).toBase58(), 1n);
    expect(first.replay).toEqual(next.replay);
    expect(first.registration).not.toEqual(next.registration);
  });

  it('matches the exact generated terminal-retirement request', () => {
    expect(encodeRegisteredRetireInstructionV1(2, 3)).toEqual(REGISTERED_RETIRE_EXAMPLE);
  });

  it('constructs buyer approval plus creation with honest maker/payer signatures and bounded reserve', () => {
    const controllerProgram = key(67);
    const maker = key(5);
    const payer = key(6);
    const market = key(4);
    const collateral = key(11);
    const intent = {
      side: 1, outcome: 1, lifecycle: 2, market: market.toBytes(), generation: 3n, nonce: 0n,
      validFrom: 0n, validThrough: 100n, maximumFill: 2_000n, limitPrice: 400_000n,
      feeBasisPoints: 25, collateralAccount: collateral.toBytes(),
    };
    const input = {
      controllerProgram: controllerProgram.toBase58(), payer: payer.toBase58(), maker: maker.toBase58(),
      market: market.toBase58(), recentBlockhash: key(91).toBase58(), intent, expectedNonce: 0n,
      route: {
        realm: key(7).toBase58(), feePolicy: key(8).toBase58(), capabilityManifest: key(9).toBase58(),
        mint: key(10).toBase58(), collateral: collateral.toBase58(), venue: key(12).toBase58(),
        tokenProgram: LEGACY_TOKEN_PROGRAM_ID.toBase58(),
      },
    };
    const plan = buildRegisteredCreateTransaction(input);
    expect(plan.instructions).toHaveLength(2);
    expect(plan.instructions[0].programId).toEqual(LEGACY_TOKEN_PROGRAM_ID);
    expect(plan.instructions[0].data).toEqual(Uint8Array.of(4, 34, 3, 0, 0, 0, 0, 0, 0));
    expect(plan.instructions[1].keys).toHaveLength(15);
    expect(plan.requiredSignerKeys).toEqual([payer.toBase58(), maker.toBase58()]);
    expect(plan.approvalAmount).toBe(802n);
    expect(plan.wireBytes.length).toBeLessThanOrEqual(1_232);
    expect(() => buildRegisteredCreateTransaction({ ...input, expectedNonce: 1n })).toThrow(/nonce is stale/);
    expect(() => buildRegisteredCreateTransaction({ ...input, route: { ...input.route, venue: input.route.mint } })).toThrow(/aliases two fixed/);
  });

  it('refuses a buyer reserve outside the 1e6 price scale', () => {
    expect(() => registeredBuyerReserve(2_000n, 1_000_001n, 25)).toThrow(/1e6 scale/);
  });

  it('constructs signer-honest fill, cancellation, and permissionless-expiry transactions', () => {
    const controllerProgram = key(67);
    const seller = observed(controllerProgram, 0, 8, 5);
    const buyer = observed(controllerProgram, 1, 9, 6);
    const route = {
      journal: key(10).toBase58(), realm: key(11).toBase58(), feePolicy: key(12).toBase58(),
      capabilityManifest: key(13).toBase58(), mint: key(14).toBase58(), source: key(6).toBase58(),
      sellerDestination: key(5).toBase58(), feeDestination: key(15).toBase58(), tokenProgram: key(16).toBase58(),
    };
    const fill = buildRegisteredFillTransaction({
      controllerProgram: controllerProgram.toBase58(), payer: key(90).toBase58(), recentBlockhash: key(91).toBase58(),
      seller, buyer, fill: 500n, executionPrice: 500_000n, route,
    });
    expect(fill.instruction.keys).toHaveLength(17);
    expect(fill.requiredSignerKeys).toEqual([key(90).toBase58()]);
    expect(fill.wireBytes.length).toBeLessThanOrEqual(1_232);

    const cancel = buildRegisteredTerminalTransaction({
      controllerProgram: controllerProgram.toBase58(), payer: key(90).toBase58(), recentBlockhash: key(91).toBase58(), state: seller, action: 'cancel', finalizedSlot: 50n,
    });
    expect(cancel.instruction.keys).toHaveLength(4);
    expect(cancel.requiredSignerKeys).toEqual([key(90).toBase58(), seller.state.maker]);

    const expire = buildRegisteredTerminalTransaction({
      controllerProgram: controllerProgram.toBase58(), payer: key(90).toBase58(), recentBlockhash: key(91).toBase58(), state: seller, action: 'expire', finalizedSlot: 101n,
    });
    expect(expire.instruction.keys).toHaveLength(3);
    expect(expire.requiredSignerKeys).toEqual([key(90).toBase58()]);
  });

  it('constructs permissionless seller and maker-authorized buyer retirement account sets', () => {
    const controllerProgram = key(67);
    const sellerOpen = observed(controllerProgram, 0, 8, 5);
    const buyerOpen = observed(controllerProgram, 1, 9, 6);
    const seller = { ...sellerOpen, state: { ...sellerOpen.state, phase: 2 } };
    const buyer = { ...buyerOpen, state: { ...buyerOpen.state, phase: 3 } };
    const payer = key(90);
    const blockhash = key(91).toBase58();
    const sellerPlan = buildRegisteredRetireTransaction({ controllerProgram: controllerProgram.toBase58(), payer: payer.toBase58(), recentBlockhash: blockhash, state: seller });
    expect(sellerPlan.instruction.keys).toHaveLength(4);
    expect(sellerPlan.requiredSignerKeys).toEqual([payer.toBase58()]);
    expect(sellerPlan.rentDestination).toBe(seller.state.maker);
    expect(sellerPlan.tokenAction).toBe('none');
    const buyerPlan = buildRegisteredRetireTransaction({ controllerProgram: controllerProgram.toBase58(), payer: payer.toBase58(), recentBlockhash: blockhash, state: buyer });
    expect(buyerPlan.instruction.keys).toHaveLength(6);
    expect(buyerPlan.instruction.keys[2]).toMatchObject({ isSigner: true, isWritable: true });
    expect(buyerPlan.instruction.keys[4].pubkey.toBase58()).toBe(new PublicKey(buyer.state.intent.collateralAccount).toBase58());
    expect(buyerPlan.instruction.keys[5].pubkey).toEqual(LEGACY_TOKEN_PROGRAM_ID);
    expect(buyerPlan.requiredSignerKeys).toEqual([payer.toBase58(), buyer.state.maker]);
    expect(buyerPlan.wireBytes.length).toBeLessThanOrEqual(1_232);
    expect(() => buildRegisteredRetireTransaction({ controllerProgram: controllerProgram.toBase58(), payer: payer.toBase58(), recentBlockhash: blockhash, state: sellerOpen })).toThrow(/terminal phase/);
    expect(() => buildRegisteredRetireTransaction({ controllerProgram: controllerProgram.toBase58(), payer: payer.toBase58(), recentBlockhash: blockhash, state: { ...seller, address: key(99).toBase58() } })).toThrow(/PDA coordinates/);
    expect(() => buildRegisteredRetireTransaction({ controllerProgram: controllerProgram.toBase58(), payer: payer.toBase58(), recentBlockhash: blockhash, state: { ...seller, state: { ...seller.state, phase: 1 } } })).toThrow(/nonzero residual/);
  });

  it('admits exact buyer delegation or canonical prior revocation and refuses substitution', () => {
    const controllerProgram = key(67);
    const buyerOpen = observed(controllerProgram, 1, 9, 6);
    const buyer = { ...buyerOpen, state: { ...buyerOpen.state, phase: 3 } };
    const base = { mint: key(14).toBase58(), owner: buyer.state.maker, amount: 2_000n, delegatedAmount: 802n, frozen: false };
    expect(registeredRetirementDelegation(buyer, { ...base, delegate: buyer.address })).toBe('revoke-registration');
    expect(registeredRetirementDelegation(buyer, { ...base, delegate: null, delegatedAmount: 0n })).toBe('already-revoked');
    expect(() => registeredRetirementDelegation(buyer, { ...base, delegate: key(99).toBase58() })).toThrow(/substituted authority/);
    expect(() => registeredRetirementDelegation(buyer, { ...base, owner: key(98).toBase58(), delegate: buyer.address })).toThrow(/persisted maker/);
    expect(() => registeredRetirementDelegation(buyer, { ...base, delegate: buyer.address, frozen: true })).toThrow(/refuse delegation revoke/);
    expect(registeredRetirementDelegation(buyer, { ...base, delegate: null, delegatedAmount: 0n, frozen: true })).toBe('already-revoked');
  });

  it('discovers only reacquired exact claim-owned state instead of trusting sliced headers', async () => {
    const controllerProgram = key(67);
    const canonical = observed(controllerProgram, 0, 8, 5);
    const bytes = encodeRegisteredIntentStateV1(canonical.state);
    const client = new SolanaRpcClient('http://127.0.0.1:8899', async (_input, init) => {
      const request = JSON.parse(String(init?.body)) as { method: string };
      if (request.method === 'getProgramAccounts') return response({ context: { slot: 55 }, value: [{
        pubkey: canonical.address,
        account: { data: [base64(bytes.slice(0, 16)), 'base64'], executable: false, lamports: 1, owner: CLAIM_PROGRAM_ID.toBase58(), space: REGISTERED_STATE_BYTES_VALUE },
      }] });
      if (request.method === 'getAccountInfo') return response({ context: { slot: 56 }, value: {
        data: [base64(bytes), 'base64'], executable: false, lamports: 1, owner: CLAIM_PROGRAM_ID.toBase58(), space: REGISTERED_STATE_BYTES_VALUE,
      } });
      throw new Error(`unexpected RPC method ${request.method}`);
    });
    const snapshot = await scanRegisteredDirectStates(client, controllerProgram.toBase58());
    expect(snapshot.states).toHaveLength(1);
    expect(snapshot.states[0].address).toBe(canonical.address);
    expect(snapshot.states[0].observedSlot).toBe('56');
    expect(snapshot.refused).toHaveLength(0);
  });

  it('refuses hostile widths, reserved bytes, stale sequence state, early expiry, and cross-Market pairing', () => {
    expect(() => decodeRegisteredIntentStateV1(REGISTERED_STATE_EXAMPLE.slice(0, -1))).toThrow(new RegExp(`exactly ${REGISTERED_STATE_BYTES_VALUE}`));
    const reserved = REGISTERED_STATE_EXAMPLE.slice();
    reserved[11] = 1;
    expect(() => decodeRegisteredIntentStateV1(reserved)).toThrow(/reserved/);
    const controllerProgram = key(67);
    const seller = observed(controllerProgram, 0, 8, 5);
    expect(() => buildRegisteredTerminalTransaction({
      controllerProgram: controllerProgram.toBase58(), payer: key(90).toBase58(), recentBlockhash: key(91).toBase58(), state: seller, action: 'expire', finalizedSlot: 100n,
    })).toThrow(/not yet admitted/);
    const buyer = observed(controllerProgram, 1, 9, 6);
    const hostileBuyer = { ...buyer, state: { ...buyer.state, intent: { ...buyer.state.intent, market: key(99).toBytes() } } };
    expect(() => buildRegisteredFillTransaction({
      controllerProgram: controllerProgram.toBase58(), payer: key(90).toBase58(), recentBlockhash: key(91).toBase58(),
      seller, buyer: hostileBuyer, fill: 500n, executionPrice: 500_000n,
      route: { journal: key(10).toBase58(), realm: key(11).toBase58(), feePolicy: key(12).toBase58(), capabilityManifest: key(13).toBase58(), mint: key(14).toBase58(), source: key(6).toBase58(), sellerDestination: key(5).toBase58(), feeDestination: key(15).toBase58(), tokenProgram: key(16).toBase58() },
    })).toThrow(/coordinates/);
  });
});
