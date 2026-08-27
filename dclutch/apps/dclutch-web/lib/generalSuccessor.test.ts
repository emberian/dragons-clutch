import { describe, expect, it } from 'vitest';
import { PublicKey } from '@solana/web3.js';

import {
  GENERAL_CANDIDATE_BYTES,
  GENERAL_EXECUTION_BYTES,
  GENERAL_PAGE_BYTES,
  GENERAL_VERIFICATION_BYTES,
  buildGeneralOuterTransaction,
  buildGeneralActionRequest,
  decodeGeneralCandidateV1,
  decodeGeneralPageV1,
  decodeGeneralPolicyV1,
  decodeGeneralSelectionV1,
  decodeGeneralVerificationV1,
  encodeGeneralRequestV1,
  previewGeneralCandidate,
} from './generalSuccessor';

function fillId(bytes: Uint8Array, offset: number, value: number): void {
  bytes.fill(value, offset, offset + 32);
}

function putU64(bytes: Uint8Array, offset: number, value: bigint): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 8).setBigUint64(0, value, true);
}

function candidateBytes(): Uint8Array {
  const bytes = new Uint8Array(GENERAL_CANDIDATE_BYTES); bytes.set(new TextEncoder().encode('DCGCAND1'));
  new DataView(bytes.buffer).setUint16(8, 1, true); bytes[10] = 2;
  fillId(bytes, 16, 1); fillId(bytes, 48, 2); fillId(bytes, 80, 3);
  new DataView(bytes.buffer).setUint32(112, 1, true); putU64(bytes, 120, 100n); putU64(bytes, 128, 40n); putU64(bytes, 136, 60n);
  return bytes;
}

function pageBytes(): Uint8Array {
  const bytes = new Uint8Array(GENERAL_PAGE_BYTES); bytes.set(new TextEncoder().encode('DCGPAGE1'));
  new DataView(bytes.buffer).setUint16(8, 1, true); bytes[10] = 2; bytes[11] = 1; fillId(bytes, 16, 1);
  new DataView(bytes.buffer).setUint32(52, 1, true);
  const row = 64; fillId(bytes, row, 4); fillId(bytes, row + 32, 5); putU64(bytes, row + 64, 7n);
  putU64(bytes, row + 72, 9n); putU64(bytes, row + 80, 1n); putU64(bytes, row + 88, 2n); putU64(bytes, row + 96, 2n);
  putU64(bytes, row + 112, 1n); putU64(bytes, row + 120, 1n);
  return bytes;
}

function selectionBytes(): Uint8Array {
  const bytes = new Uint8Array(128); bytes.set(new TextEncoder().encode('DCGSELC1'));
  new DataView(bytes.buffer).setUint16(8, 1, true); bytes[11] = 1; fillId(bytes, 16, 3); fillId(bytes, 48, 6); fillId(bytes, 80, 1); putU64(bytes, 112, 8n);
  return bytes;
}

function publicKey(value: number): PublicKey { return new PublicKey(new Uint8Array(32).fill(value)); }
function idBytes(value: string): Uint8Array { return Uint8Array.from(value.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16)); }
function pda(program: PublicKey, domain: string, ...seeds: Uint8Array[]): string { return PublicKey.findProgramAddressSync([new TextEncoder().encode(domain), ...seeds], program)[0].toBase58(); }

describe('generated General browser profile', () => {
  it('decodes exact candidate/page data and recomputes quote once per order', () => {
    const candidate = decodeGeneralCandidateV1(candidateBytes());
    const page = decodeGeneralPageV1(pageBytes());
    const preview = previewGeneralCandidate(candidate, [page]);
    expect(preview).toMatchObject({ complete: true, valid: true, filledLots: 2n, quoteInputs: 2n, quoteOutputs: 0n, quoteSurplus: 0n, completeSetMove: 'mint 2 complete sets' });
    expect(preview.orders[0]).toMatchObject({ fragments: 1, expectedQuoteDebit: 2n, submittedQuoteDebit: 2n, valid: true });
  });

  it('refuses per-fragment quote rounding and inactive physical storage', () => {
    const hostile = pageBytes(); putU64(hostile, 64 + 96, 1n);
    const preview = previewGeneralCandidate(decodeGeneralCandidateV1(candidateBytes()), [decodeGeneralPageV1(hostile)]);
    expect(preview.valid).toBe(false); expect(preview.orders[0].refusal).toMatch(/sole candidate-wide rounding/);
    const padding = pageBytes(); padding[GENERAL_PAGE_BYTES - 1] = 1;
    expect(() => decodeGeneralPageV1(padding)).toThrow(/inactive execution capacity/);
  });

  it('interprets policy order and rejects nondeterministic endings', () => {
    const bytes = new Uint8Array(64); bytes.set(new TextEncoder().encode('DCGPOLY1')); new DataView(bytes.buffer).setUint16(8, 1, true);
    bytes[10] = 3; fillId(bytes, 16, 6); bytes.set([0, 1, 2], 48);
    expect(decodeGeneralPolicyV1(bytes).criteria).toEqual(['maximize filled lots', 'minimize quote surplus', 'minimize candidate ID']);
    bytes[50] = 1; expect(() => decodeGeneralPolicyV1(bytes)).toThrow(/final tie-break/);
  });

  it('builds the exact generated request and names the still-unproven physical boundary', () => {
    const candidate = previewGeneralCandidate(decodeGeneralCandidateV1(candidateBytes()), [decodeGeneralPageV1(pageBytes())]);
    const selection = decodeGeneralSelectionV1(selectionBytes());
    const request = buildGeneralActionRequest({ action: 'consider', selection: { ...selection, closed: false, bestCandidateId: null }, candidate });
    expect(request.bytes).toHaveLength(64); expect([...request.bytes.slice(8, 17)]).toEqual([1, 0, 0, 0, 0, 0, 0, 0, 0]);
    expect(request.transactionAvailable).toBe(false); expect(request.unavailableReason).toMatch(/Registry activation/);
    expect(() => encodeGeneralRequestV1('freeze', 0n, candidate.candidate.candidateId, 0)).toThrow(/noncanonical/);
  });

  it('derives every streamed coordinate and refuses physically impossible cursor effects', () => {
    const candidate = previewGeneralCandidate(decodeGeneralCandidateV1(candidateBytes()), [decodeGeneralPageV1(pageBytes())]);
    const selection = decodeGeneralSelectionV1(selectionBytes());
    expect(buildGeneralActionRequest({ action: 'freeze', selection }).candidateId).toBeNull();
    expect(buildGeneralActionRequest({ action: 'initialize-settlement', selection: { ...selection, closed: true }, candidate }).candidateId).toBe(candidate.candidate.candidateId);
    const settlement = { phase: 'collecting' as const, outcomeCount: 2, candidateId: candidate.candidate.candidateId, pageCount: 1, nextPage: 0, nextExecution: 0, revision: 3n, claimInventory: [0n, 0n], quoteInventory: 0n, quoteSurplusPaid: 0n };
    expect(buildGeneralActionRequest({ action: 'collect', settlement, candidate })).toMatchObject({ pageIndex: 0, expectedRevision: 3n });
    expect(buildGeneralActionRequest({ action: 'materialize', settlement: { ...settlement, phase: 'materializing', quoteInventory: 2n }, candidate }).action).toBe('materialize');
    expect(buildGeneralActionRequest({ action: 'distribute', settlement: { ...settlement, phase: 'distributing', claimInventory: [2n, 2n], quoteInventory: 0n }, candidate }).action).toBe('distribute');
    expect(buildGeneralActionRequest({ action: 'close', settlement: { ...settlement, phase: 'ready-to-close' }, candidate }).action).toBe('close');
    expect(() => buildGeneralActionRequest({ action: 'materialize', settlement: { ...settlement, phase: 'materializing', quoteInventory: 1n }, candidate })).toThrow(/exceeds collected quote/);
    expect(() => buildGeneralActionRequest({ action: 'close', settlement: { ...settlement, phase: 'ready-to-close', claimInventory: [1n, 0n] }, candidate })).toThrow(/claim inventory remains/);
  });

  it('decodes the 960-byte verification cursor and places the row coordinate at byte 60', () => {
    const bytes = new Uint8Array(GENERAL_VERIFICATION_BYTES); bytes.set(new TextEncoder().encode('DCGVERF1')); new DataView(bytes.buffer).setUint16(8, 1, true); bytes.set(candidateBytes(), 16);
    expect(decodeGeneralVerificationV1(bytes)).toMatchObject({ nextPage: 0, revision: 0n, hasCurrentOrder: false });
    const request = encodeGeneralRequestV1('collect', 9n, '01'.repeat(32), 3, 7);
    expect(request[60]).toBe(7); expect([...request.slice(61)]).toEqual([0, 0, 0]);
  });

  it('refuses ungrouped rows and fixed-width arithmetic overflow', () => {
    const unordered = pageBytes(); unordered[11] = 2; unordered.set(unordered.slice(64, 64 + GENERAL_EXECUTION_BYTES), 64 + GENERAL_EXECUTION_BYTES); fillId(unordered, 64 + GENERAL_EXECUTION_BYTES, 3);
    expect(previewGeneralCandidate(decodeGeneralCandidateV1(candidateBytes()), [decodeGeneralPageV1(unordered)]).refusal).toMatch(/globally grouped/);
    const overflow = pageBytes(); putU64(overflow, 64 + 112, 18_446_744_073_709_551_615n);
    expect(previewGeneralCandidate(decodeGeneralCandidateV1(candidateBytes()), [decodeGeneralPageV1(overflow)]).refusal).toMatch(/fixed-width/);
  });

  it('builds the exact packet-bounded 12-account Consider transaction', () => {
    const candidate = decodeGeneralCandidateV1(candidateBytes()); const page = decodeGeneralPageV1(pageBytes()); const preview = previewGeneralCandidate(candidate, [page]);
    const policyBytes = new Uint8Array(64); policyBytes.set(new TextEncoder().encode('DCGPOLY1')); new DataView(policyBytes.buffer).setUint16(8, 1, true); policyBytes[10] = 1; fillId(policyBytes, 16, 6); policyBytes[48] = 2;
    const policy = decodeGeneralPolicyV1(policyBytes); const request = buildGeneralActionRequest({ action: 'consider', selection: null, verification: null, candidate: preview });
    const program = publicKey(21); const market = publicKey(22); const registry = publicKey(23); const cache = publicKey(24); const programData = publicKey(25); const payer = publicKey(26);
    const candidateId = idBytes(candidate.candidateId); const batchId = idBytes(candidate.batchId); const policyId = idBytes(policy.policyId); const pageIndex = new Uint8Array(4);
    const role = { artifactReleaseId: '01'.repeat(32), program: program.toBase58(), loaderProgram: publicKey(27).toBase58(), programData: programData.toBase58(), semanticReleaseId: '02'.repeat(32), elfDigest: '03'.repeat(32), deploymentSlot: 1n, upgradeAuthority: null, bytes: new Uint8Array(248) };
    const activation = { releaseSetId: new Uint8Array(32).fill(7), roles: { core: role, claims: role, trading: role, resolution: role, custody: role } };
    const accounts = {
      market: market.toBase58(), activationCache: cache.toBase58(), registryProgram: registry.toBase58(), tradingProgram: program.toBase58(), tradingProgramData: programData.toBase58(),
      selection: pda(program, 'dclutch:general-selection:v1', market.toBytes(), batchId), verification: pda(program, 'dclutch:general-verification:v1', market.toBytes(), candidateId), certificate: pda(program, 'dclutch:general-certificate:v1', market.toBytes(), candidateId), candidate: pda(program, 'dclutch:general-candidate:v1', market.toBytes(), candidateId), policy: pda(program, 'dclutch:general-policy:v1', market.toBytes(), policyId), page: pda(program, 'dclutch:general-page:v1', market.toBytes(), candidateId, pageIndex), incumbentCertificate: market.toBase58(),
    };
    const transaction = { action: 'consider' as const, recentBlockhash: publicKey(28).toBase58(), activation, request, candidate, policy, selection: null, accounts };
    const built = buildGeneralOuterTransaction({ ...transaction, payer: payer.toBase58() });
    expect(built.accountCount).toBe(12); expect(built.wireBytes.length).toBeLessThanOrEqual(1_232); expect(built.requiredSigners).toEqual([payer.toBase58()]);
    expect(() => buildGeneralOuterTransaction({ ...transaction, payer: market.toBase58() })).toThrow(/escalate/);
    expect(() => buildGeneralOuterTransaction({ ...transaction, payer: payer.toBase58(), accounts: { ...accounts, page: publicKey(29).toBase58() } })).toThrow(/candidate page/);
  });
});
