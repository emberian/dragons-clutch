import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import type {
  PreparedWalletTerminalPayoutV3,
  WalletTerminalPayoutManifestV3,
  WalletTerminalPayoutReportV3,
  WalletTerminalPayoutRouteV3,
} from '@dclutch/sdk/walletTerminalPayoutV3';
import {
  encodeClaimsCustodyReplayRequestV1,
  encodeExpectedCustodyRequestV1,
  type ClaimsCustodyReplayPlanV1,
} from '@dclutch/sdk/claimsCustodyReplay';
import {
  CUSTODY_REPLAY_BYTES_V1,
  CUSTODY_REPLAY_CALLER_PROGRAM_OFFSET_V1,
  CUSTODY_REPLAY_CALLER_ROLE_OFFSET_V1,
  CUSTODY_REPLAY_CONTEXT_OFFSET_V1,
  CUSTODY_REPLAY_GENERATION_OFFSET_V1,
  CUSTODY_REPLAY_MAGIC_V1,
  CUSTODY_REPLAY_MARKET_OFFSET_V1,
  CUSTODY_REPLAY_NEXT_REVISION_OFFSET_V1,
  CUSTODY_REPLAY_REALM_OFFSET_V1,
  CUSTODY_REPLAY_RELEASE_SET_OFFSET_V1,
  CUSTODY_REPLAY_RENT_REFUND_OFFSET_V1,
  CUSTODY_REPLAY_STATUS_OFFSET_V1,
  CUSTODY_REPLAY_VERSION_OFFSET_V1,
} from '@dclutch/sdk/generated/claimsCustodyReplayV1';
import type { RpcAccount, TransactionMetaObservation } from '@dclutch/sdk/rpc';
import {
  Keypair,
  PublicKey,
  SystemProgram,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  archivePayoutOperationJournalV1,
  archiveReplayOperationJournalV1,
  authenticateCompletedCampaignEvidenceV1,
  finalizePayoutOperationV1,
  finalizeReplayOperationV1,
  loadPayoutOperationJournalV1,
  loadReplayOperationJournalV1,
  markPayoutOperationSubmittedV1,
  markReplayOperationSubmittedV1,
  parseWalletTerminalPayoutPlanInputV1,
  restorePayoutOperationJournalV1,
  restoreReplayOperationJournalV1,
  signPayoutPlanV1,
  signReplayOperationV1,
  transactionSignatureV1,
  writeUnsignedPayoutOperationJournalV1,
  writeUnsignedReplayOperationJournalV1,
  type PayoutOperationJournalV1,
} from '../src/payoutCompletion';

const key = (byte: number) => new PublicKey(new Uint8Array(32).fill(byte)).toBase58();
const digest = (byte: number) => byte.toString(16).padStart(2, '0').repeat(32);

function inputValue() {
  const owner = key(2);
  return {
    format: 'dclutch-wallet-terminal-payout-plan-input-v1',
    market: key(1), owner, recipientOwner: owner, recipient: key(3),
    collateralMint: key(4), tokenProgram: key(5), quantity: '7', claimIndex: 1,
    transferIndex: 0, parentContext: digest(6), custodyContext: digest(7), releaseSet: digest(8),
    programs: { registry: key(9), core: key(10), claims: key(11), custody: key(12) },
    records: {
      realm: digest(13), product: digest(14), resultDomain: digest(15), portfolio: digest(16),
      productBasis: digest(17), compositionDescriptor: digest(18), compositionGraph: digest(19),
      compositionTranslation: digest(20), compositionExposure: digest(21), terminalRecord: digest(22),
    },
  };
}

function evidenceValue(planBytes: Uint8Array) {
  return {
    schema: 'dclutch-local-successor-run-evidence-v2', rpc_url: 'https://api.devnet.solana.com/',
    ledger: '/tmp/ledger', validator_log: '/tmp/validator.log',
    plan_sha256: createHash('sha256').update(planBytes).digest('hex'),
    core_upgrade_authority_pubkey: key(40), private_key_persisted: false,
    keypair_derivation: 'random-per-run', keypair_seed_sha256: null,
    foundingCustodyContext: digest(7), directSelectedManifestEntryIndex: 0,
    completed: ['founding', 'open'], transactions: [],
    accounts: {
      market: {
        address: key(1), owner: key(11), lamports: 1, executable: false, data_len: 1,
        data_sha256: digest(41), account_sha256: digest(42),
      },
    },
    remaining_execution_seam: 'none',
  };
}

function route(): WalletTerminalPayoutRouteV3 {
  const names = [
    'aggregate', 'linkedBasisRaw', 'linkedBasisStaging', 'productRaw', 'productStaging',
    'resultDomainRaw', 'resultDomainStaging', 'portfolioRaw', 'portfolioStaging', 'market',
    'activationCache', 'registryProgram', 'claimsProgram', 'claimsProgramData', 'coreProgram',
    'coreProgramData', 'position', 'exposureRaw', 'exposureStaging', 'custodyProgram',
    'terminalCoordinateRaw', 'terminalCoordinateStaging', 'realmRaw', 'realmStaging',
    'custodyReplay', 'collateralMint', 'hoard', 'recipient', 'custodyAuthority', 'tokenProgram',
  ] as const;
  const value = Object.fromEntries(names.map((name, offset) => [name, key(30 + offset)]));
  return value as WalletTerminalPayoutRouteV3;
}

function preparedFixture() {
  const signer = Keypair.fromSeed(new Uint8Array(32).fill(23));
  const payoutRoute = route();
  const instruction = new TransactionInstruction({
    programId: SystemProgram.programId,
    keys: [],
    data: Buffer.alloc(0),
  });
  const transaction = new VersionedTransaction(new TransactionMessage({
    payerKey: signer.publicKey,
    recentBlockhash: key(90),
    instructions: [instruction],
  }).compileToV0Message());
  const request = {
    releaseSet: digest(1), market: payoutRoute.market, realm: digest(2), parentContext: digest(3),
    productRecordDigest: digest(4), exposureId: digest(5), exposureDigest: digest(6),
    terminalRecordDigest: digest(7), owner: signer.publicKey.toBase58(), position: payoutRoute.position,
    recipientOwner: signer.publicKey.toBase58(), recipient: payoutRoute.recipient,
    claimsProgram: payoutRoute.claimsProgram, custodyProgram: payoutRoute.custodyProgram,
    collateralMint: payoutRoute.collateralMint, tokenProgram: payoutRoute.tokenProgram,
    semanticBasisId: digest(8), linkedBasisRecordDigest: digest(9), generation: '1',
    expectedMarketRevision: '2', expectedPositionRevision: '3', expectedCustodyRevision: '4',
    quantity: '7', claimIndex: 1, transferIndex: 0,
  };
  const report = {
    observedSlot: '40', route: payoutRoute, payout: '7', instruction,
    request,
    requestBytes: new Uint8Array([1]), requestDigest: new Uint8Array(32).fill(1),
    signedPacket: new Uint8Array([2]), signedPacketDigest: new Uint8Array(32).fill(2),
    signedTableDigest: new Uint8Array(32).fill(3), custodyCaller: payoutRoute.claimsProgram,
    custodyRequestDigest: new Uint8Array(32).fill(4),
    preAggregateBytes: new Uint8Array([5]), prePositionBytes: new Uint8Array([6]),
    preCustodyReplayBytes: new Uint8Array([7]), preHoardTokenBytes: new Uint8Array([8]),
    preRecipientTokenBytes: new Uint8Array([9]),
  } as unknown as WalletTerminalPayoutReportV3;
  const plan = Object.freeze({
    transaction,
    wireBytes: transaction.serialize(),
    requiredSigners: Object.freeze([signer.publicKey.toBase58()]),
    report,
    lookupTable: key(91),
  }) as PreparedWalletTerminalPayoutV3;
  const manifest = {
    format: 'dclutch-wallet-terminal-payout-v3',
    route: payoutRoute,
    request,
    custodyContext: digest(5), signedPacketBase64: 'AA==', payout: '7', lookupTable: key(91),
  } as unknown as WalletTerminalPayoutManifestV3;
  return { signer, plan, manifest };
}

function putU16(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 2).setUint16(0, value, true);
}

function putU64(bytes: Uint8Array, offset: number, value: bigint): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 8).setBigUint64(0, value, true);
}

function hashBytes(...parts: ReadonlyArray<Uint8Array>): Uint8Array {
  const hash = createHash('sha256');
  for (const part of parts) hash.update(part);
  return new Uint8Array(hash.digest());
}

function littleU64(value: bigint): Uint8Array {
  const bytes = new Uint8Array(8); putU64(bytes, 0, value); return bytes;
}

async function replayFixture() {
  const signer = Keypair.fromSeed(new Uint8Array(32).fill(71));
  const market = key(61); const claims = key(62); const custody = key(63); const registry = key(64);
  const replay = key(65); const aggregate = key(66); const rent = 1_000n;
  const release = new Uint8Array(32).fill(6); const realm = new Uint8Array(32).fill(7);
  const context = new Uint8Array(32).fill(8);
  const custodyRequestBytes = await encodeExpectedCustodyRequestV1({
    releaseSet: release, market: new PublicKey(market).toBytes(), realm, context,
    claimsProgram: new PublicKey(claims).toBytes(), payer: signer.publicKey.toBytes(),
    generation: 9n, rentLamports: rent,
  });
  const instructionData = encodeClaimsCustodyReplayRequestV1(market);
  const instruction = new TransactionInstruction({
    programId: new PublicKey(claims),
    keys: [
      { pubkey: new PublicKey(replay), isSigner: false, isWritable: true },
      { pubkey: new PublicKey(aggregate), isSigner: false, isWritable: false },
      { pubkey: new PublicKey(custody), isSigner: false, isWritable: false },
      { pubkey: new PublicKey(registry), isSigner: false, isWritable: false },
    ],
    data: Buffer.from(instructionData),
  });
  const transaction = new VersionedTransaction(new TransactionMessage({
    payerKey: signer.publicKey, recentBlockhash: key(90), instructions: [instruction],
  }).compileToLegacyMessage());
  const requestDigest = hashBytes(custodyRequestBytes);
  const plan = Object.freeze({
    marketAddress: market, aggregateAddress: aggregate,
    aggregate: { registryProgram: registry }, replayAddress: replay,
    callerAuthorityAddress: key(67), activationCacheAddress: key(68), claimsProgramDataAddress: key(69),
    realmRecordAddress: key(70), realmStagingAddress: key(72), payer: signer.publicKey.toBase58(),
    rentLamports: rent.toString(), custodyRequestBytes,
    custodyRequestDigestHex: Buffer.from(requestDigest).toString('hex'), instructionData,
    transaction, wireBytes: transaction.serialize(), requiredSigners: Object.freeze([signer.publicKey.toBase58()]),
  }) as unknown as ClaimsCustodyReplayPlanV1;

  const poststate = hashBytes(
    new TextEncoder().encode('dclutch:custody-poststate:v1'), requestDigest,
    new PublicKey(replay).toBytes(), new PublicKey(replay).toBytes(),
    littleU64(0n), littleU64(0n), littleU64(0n), littleU64(0n), littleU64(rent),
  );
  const replayBytes = new Uint8Array(CUSTODY_REPLAY_BYTES_V1);
  replayBytes.set(CUSTODY_REPLAY_MAGIC_V1, 0); putU16(replayBytes, CUSTODY_REPLAY_VERSION_OFFSET_V1, 1);
  replayBytes[CUSTODY_REPLAY_STATUS_OFFSET_V1] = 1; replayBytes[CUSTODY_REPLAY_CALLER_ROLE_OFFSET_V1] = 1;
  replayBytes.set(release, CUSTODY_REPLAY_RELEASE_SET_OFFSET_V1);
  replayBytes.set(new PublicKey(market).toBytes(), CUSTODY_REPLAY_MARKET_OFFSET_V1);
  replayBytes.set(realm, CUSTODY_REPLAY_REALM_OFFSET_V1); replayBytes.set(context, CUSTODY_REPLAY_CONTEXT_OFFSET_V1);
  replayBytes.set(new PublicKey(claims).toBytes(), CUSTODY_REPLAY_CALLER_PROGRAM_OFFSET_V1);
  replayBytes.set(signer.publicKey.toBytes(), CUSTODY_REPLAY_RENT_REFUND_OFFSET_V1);
  putU64(replayBytes, CUSTODY_REPLAY_NEXT_REVISION_OFFSET_V1, 1n);
  putU64(replayBytes, CUSTODY_REPLAY_GENERATION_OFFSET_V1, 9n);
  replayBytes.set(requestDigest, 224); replayBytes.set(poststate, 256);

  const receipt = new Uint8Array(384);
  receipt.set(new TextEncoder().encode('DCLCUSC1'), 0); putU16(receipt, 8, 1);
  receipt[10] = 0; receipt[11] = 1;
  receipt.set(release, 16); receipt.set(new PublicKey(market).toBytes(), 48); receipt.set(context, 80);
  receipt.set(custodyRequestBytes.slice(304, 336), 112); receipt.set(requestDigest, 144);
  putU64(receipt, 248, 1n); putU64(receipt, 296, rent); receipt.set(poststate, 304);
  receipt.set(hashBytes(replayBytes), 336);
  return { signer, plan, programs: { claims, custody, registry }, replayBytes, receipt, rent };
}

function material(owner: string, byte: number): RpcAccount {
  return Object.freeze({ data: new Uint8Array([byte]), executable: false, lamports: '1', owner, space: 1 });
}

describe('CLI payout completion boundary', () => {
  it('hostile-decodes only the exact flat Rust projection and refuses coordinate aliases', () => {
    const value = inputValue();
    const source = JSON.stringify(value);
    expect(parseWalletTerminalPayoutPlanInputV1(source)).toEqual(value);
    expect(() => parseWalletTerminalPayoutPlanInputV1(source.replace(
      `"market":"${value.market}"`,
      `"market":"${value.market}","market":"${value.market}"`,
    ))).toThrow(/duplicate JSON object key/);
    expect(() => parseWalletTerminalPayoutPlanInputV1(source.replace(
      `"claims":"${value.programs.claims}"`,
      `"claims":"${value.programs.claims}","cl\\u0061ims":"${value.programs.claims}"`,
    ))).toThrow(/duplicate JSON object key/);
    expect(() => parseWalletTerminalPayoutPlanInputV1(`${source}{}`)).toThrow(/trailing data/);
    expect(() => parseWalletTerminalPayoutPlanInputV1(JSON.stringify({ ...value, ignored: true })))
      .toThrow(/missing or unknown fields/);
    expect(() => parseWalletTerminalPayoutPlanInputV1(JSON.stringify({ ...value, recipientOwner: key(99) })))
      .toThrow(/differs from its owner/);
    expect(() => parseWalletTerminalPayoutPlanInputV1(JSON.stringify({ ...value, quantity: '07' })))
      .toThrow(/canonical decimal/);
    expect(() => parseWalletTerminalPayoutPlanInputV1(JSON.stringify({
      ...value, records: { ...value.records, parallelDtoTruth: digest(99) },
    }))).toThrow(/missing or unknown fields/);
  });

  it('refuses duplicate and trailing campaign evidence before normalization and enforces real text and array bounds', () => {
    const plan = Buffer.from('{"plan":"exact"}\n');
    const valid = evidenceValue(plan);
    expect(() => authenticateCompletedCampaignEvidenceV1(plan, Buffer.from(JSON.stringify(valid)))).not.toThrow();
    expect(() => authenticateCompletedCampaignEvidenceV1(plan, Buffer.from(
      '{"schema":"first","schema":"second"}',
    ))).toThrow(/duplicate JSON object key/);
    expect(() => authenticateCompletedCampaignEvidenceV1(plan, Buffer.from(
      '{"accounts":{"market":{"owner":"first","\\u006fwner":"second"}}}',
    ))).toThrow(/duplicate JSON object key/);
    expect(() => authenticateCompletedCampaignEvidenceV1(plan, Buffer.from(`${JSON.stringify(valid)} null`)))
      .toThrow(/trailing data/);
    expect(() => authenticateCompletedCampaignEvidenceV1(plan, Buffer.from(JSON.stringify({
      ...valid, ledger: `/${'x'.repeat(4_096)}`,
    })))).toThrow(/4096 bytes/);
    expect(() => authenticateCompletedCampaignEvidenceV1(plan, Buffer.from(JSON.stringify({
      ...valid, completed: Array.from({ length: 513 }, (_, index) => `stage-${index}`),
    })))).toThrow(/completed-stage list/);
    expect(() => authenticateCompletedCampaignEvidenceV1(plan, Buffer.from(JSON.stringify({
      ...valid, transactions: Array.from({ length: 4_097 }, () => null),
    })))).toThrow(/4096-row bound/);
    expect(() => authenticateCompletedCampaignEvidenceV1(plan, Buffer.from(JSON.stringify({
      ...valid,
      transactions: [{
        label: 'one', signature: transactionSignatureV1(new Uint8Array(64).fill(1)), slot: 1,
        transaction_metadata_available: true, fee_lamports: 1, fee_only_balance_change: true,
        compute_units_consumed: 1, error: null, logs: Array.from({ length: 513 }, () => 'log'),
      }],
    })))).toThrow(/inexact finalized evidence/);
  });

  it('persists exact intent and plan digests before signing, then preserves submitted ambiguity', async () => {
    const directory = mkdtempSync(join(tmpdir(), 'dclutch-payout-journal-test-'));
    const path = join(directory, 'payout.json');
    try {
      const { signer, plan, manifest } = preparedFixture();
      const unsigned = writeUnsignedPayoutOperationJournalV1(path, key(1), manifest, plan);
      expect(loadPayoutOperationJournalV1(path)).toEqual(unsigned);
      expect(unsigned).toMatchObject({ phase: 'unsigned', signature: null, market: manifest.request.market });
      const source = readFileSync(path, 'utf8');
      writeFileSync(path, source.replace(
        '"operation":"wallet-terminal-payout-v3"',
        '"operation":"wallet-terminal-payout-v3","operation":"wallet-terminal-payout-v3"',
      ));
      expect(() => loadPayoutOperationJournalV1(path)).toThrow(/duplicate JSON object key/);
      writeFileSync(path, `${source}{}`);
      expect(() => loadPayoutOperationJournalV1(path)).toThrow(/trailing data/);
      writeFileSync(path, source);
      const duplicateIntent = JSON.parse(source) as Record<string, unknown>;
      duplicateIntent.intent = '{"request":"first","request":"second"}';
      duplicateIntent.intentDigest = createHash('sha256').update(String(duplicateIntent.intent)).digest('hex');
      writeFileSync(path, JSON.stringify(duplicateIntent));
      expect(() => loadPayoutOperationJournalV1(path)).toThrow(/duplicate JSON object key/);
      writeFileSync(path, source);
      const changed = JSON.parse(source) as Record<string, unknown>;
      changed.intent = `${String(changed.intent)} `;
      writeFileSync(path, JSON.stringify(changed));
      expect(() => loadPayoutOperationJournalV1(path)).toThrow(/intent or plan bytes/);
      writeFileSync(path, source);

      const savedPlan = unsigned.plan;
      const duplicateSavedPlan = savedPlan.replace(
        '"format":"dclutch-wallet-terminal-payout-journal-plan-v1"',
        '"format":"dclutch-wallet-terminal-payout-journal-plan-v1","format":"dclutch-wallet-terminal-payout-journal-plan-v1"',
      );
      await expect(restorePayoutOperationJournalV1({ ...unsigned, plan: duplicateSavedPlan }))
        .rejects.toThrow(/duplicate JSON object key/);
      await expect(restorePayoutOperationJournalV1({ ...unsigned, plan: `${savedPlan}[]` }))
        .rejects.toThrow(/trailing data/);

      const signed = signPayoutPlanV1(plan, signer);
      const submitted = markPayoutOperationSubmittedV1(path, unsigned, signed.signature);
      expect(submitted).toMatchObject({ phase: 'submitted', signature: signed.signature });
      expect(() => archivePayoutOperationJournalV1(path, submitted, 'discarded')).toThrow(/cannot be discarded/);
      expect(loadPayoutOperationJournalV1(path)).toEqual(submitted);
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });

  it('journals Claims replay bytes before signing and refuses duplicate, trailing, and submitted discard ambiguity', async () => {
    const directory = mkdtempSync(join(tmpdir(), 'dclutch-replay-journal-test-'));
    const path = join(directory, 'payout.json.claims-replay.json');
    try {
      const fixture = await replayFixture();
      const unsigned = writeUnsignedReplayOperationJournalV1(path, key(1), fixture.plan, fixture.programs);
      expect(loadReplayOperationJournalV1(path)).toEqual(unsigned);
      expect(restoreReplayOperationJournalV1(unsigned)).toMatchObject({
        market: fixture.plan.marketAddress, owner: fixture.plan.payer, replay: fixture.plan.replayAddress,
      });
      const source = readFileSync(path, 'utf8');
      writeFileSync(path, source.replace(
        '"operation":"claims-custody-replay-create-v1"',
        '"operation":"claims-custody-replay-create-v1","operation":"claims-custody-replay-create-v1"',
      ));
      expect(() => loadReplayOperationJournalV1(path)).toThrow(/duplicate JSON object key/);
      writeFileSync(path, `${source} null`);
      expect(() => loadReplayOperationJournalV1(path)).toThrow(/trailing data/);
      const nested = JSON.parse(source) as Record<string, unknown>;
      nested.intent = String(nested.intent).replace(
        '"format":"dclutch-claims-custody-replay-journal-intent-v1"',
        '"format":"dclutch-claims-custody-replay-journal-intent-v1","format":"dclutch-claims-custody-replay-journal-intent-v1"',
      );
      nested.intentDigest = createHash('sha256').update(String(nested.intent)).digest('hex');
      writeFileSync(path, JSON.stringify(nested));
      expect(() => loadReplayOperationJournalV1(path)).toThrow(/duplicate JSON object key/);
      writeFileSync(path, source);

      const duplicatePlan = JSON.parse(source) as Record<string, unknown>;
      duplicatePlan.plan = String(duplicatePlan.plan).replace(
        '"format":"dclutch-claims-custody-replay-journal-plan-v1"',
        '"format":"dclutch-claims-custody-replay-journal-plan-v1","format":"dclutch-claims-custody-replay-journal-plan-v1"',
      );
      duplicatePlan.planDigest = createHash('sha256').update(String(duplicatePlan.plan)).digest('hex');
      writeFileSync(path, JSON.stringify(duplicatePlan));
      expect(() => loadReplayOperationJournalV1(path)).toThrow(/duplicate JSON object key/);
      writeFileSync(path, source);

      const savedPlan = JSON.parse(unsigned.plan) as Record<string, unknown>;
      const request = Buffer.from(String(savedPlan.custodyRequestBase64), 'base64'); request[176] = 1;
      savedPlan.custodyRequestBase64 = request.toString('base64');
      const substitutedDigest = createHash('sha256').update(request).digest('hex');
      const savedIntent = JSON.parse(unsigned.intent) as Record<string, unknown>;
      savedIntent.custodyRequestDigest = substitutedDigest;
      expect(() => restoreReplayOperationJournalV1({
        ...unsigned,
        operationDigest: substitutedDigest,
        intent: JSON.stringify(savedIntent),
        plan: JSON.stringify(savedPlan),
      }))
        .toThrow(/exact InitializeReplay coordinates/);

      const restored = restoreReplayOperationJournalV1(unsigned);
      const signed = signReplayOperationV1(restored, fixture.signer);
      const submitted = markReplayOperationSubmittedV1(path, unsigned, signed.signature);
      expect(submitted.phase).toBe('submitted');
      expect(() => archiveReplayOperationJournalV1(path, submitted, 'discarded')).toThrow(/cannot be discarded/);
      expect(loadReplayOperationJournalV1(path)).toEqual(submitted);
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });

  it('finalizes Claims replay only with the exact packet, Custody receipt, rent movement, and replay poststate', async () => {
    const directory = mkdtempSync(join(tmpdir(), 'dclutch-replay-finalize-test-'));
    const path = join(directory, 'payout.json.claims-replay.json');
    try {
      const fixture = await replayFixture();
      const unsigned = writeUnsignedReplayOperationJournalV1(path, key(1), fixture.plan, fixture.programs);
      const restored = restoreReplayOperationJournalV1(unsigned);
      const signed = signReplayOperationV1(restored, fixture.signer);
      const submitted = markReplayOperationSubmittedV1(path, unsigned, signed.signature);
      const addresses = restored.transaction.message.staticAccountKeys.map((address) => address.toBase58());
      const ownerIndex = addresses.indexOf(restored.owner); const replayIndex = addresses.indexOf(restored.replay);
      const pre = addresses.map((_, index) => index === ownerIndex ? '10000' : index === replayIndex ? '0' : '1');
      const post = addresses.map((_, index) => index === ownerIndex
        ? String(10_000n - fixture.rent - 5n) : index === replayIndex ? fixture.rent.toString() : '1');
      const meta: TransactionMetaObservation = Object.freeze({
        signature: signed.signature, slot: '50', blockTime: null, succeeded: true, errorText: null,
        feeLamports: '5', accountAddresses: Object.freeze(addresses),
        preBalances: Object.freeze(pre), postBalances: Object.freeze(post), logMessages: Object.freeze([]),
        returnData: Object.freeze({ programId: fixture.programs.custody, data: fixture.receipt }),
        transactionBytes: signed.wireBytes,
      });
      const replayAccount = material(fixture.programs.custody, 1);
      const exactReplayAccount: RpcAccount = Object.freeze({
        ...replayAccount, data: fixture.replayBytes, space: fixture.replayBytes.length,
        lamports: fixture.rent.toString(),
      });
      const client = (changedMeta: TransactionMetaObservation = meta, account: RpcAccount = exactReplayAccount) => ({
        transaction: async () => changedMeta,
        finalizedSlot: async () => '51',
        multipleAccounts: async () => ({
          slot: '51', accounts: Object.freeze([{ address: restored.replay, account }]),
        }),
      });
      await expect(finalizeReplayOperationV1(client(), submitted, restored)).resolves.toEqual({
        signature: signed.signature, observedSlot: '51', replay: restored.replay,
      });
      await expect(finalizeReplayOperationV1(client({ ...meta, transactionBytes: restored.wireBytes }), submitted, restored))
        .rejects.toThrow(/packet differs/);
      await expect(finalizeReplayOperationV1(client({ ...meta, returnData: null }), submitted, restored))
        .rejects.toThrow(/Custody-produced/);
      await expect(finalizeReplayOperationV1(client({
        ...meta, returnData: { programId: fixture.programs.claims, data: fixture.receipt },
      }), submitted, restored)).rejects.toThrow(/Custody-produced/);
      const wrongReceipt = new Uint8Array(fixture.receipt); wrongReceipt[144] ^= 1;
      await expect(finalizeReplayOperationV1(client({
        ...meta, returnData: { programId: fixture.programs.custody, data: wrongReceipt },
      }), submitted, restored)).rejects.toThrow(/request or poststate/);
      await expect(finalizeReplayOperationV1(client({
        ...meta, postBalances: Object.freeze(post.map((balance, index) => index === replayIndex ? '999' : balance)),
      }), submitted, restored)).rejects.toThrow(/fee-plus-rent/);
      const wrongReplay = new Uint8Array(fixture.replayBytes); wrongReplay[224] ^= 1;
      await expect(finalizeReplayOperationV1(client(meta, { ...exactReplayAccount, data: wrongReplay }), submitted, restored))
        .rejects.toThrow(/exact digests/);
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });

  it('finalizes only the exact signed packet, Claims return data, fee balances, and owned poststate', async () => {
    const { signer, plan } = preparedFixture();
    const signed = signPayoutPlanV1(plan, signer);
    const journal = Object.freeze({
      format: 'dclutch-client-operation-journal-v1', operation: 'wallet-terminal-payout-v3',
      clusterGenesis: key(1), market: plan.report.route.market, owner: signer.publicKey.toBase58(),
      operationDigest: digest(1), intentDigest: digest(2), planDigest: digest(3),
      intent: '{}', plan: '{}', phase: 'submitted', signature: signed.signature,
    }) as PayoutOperationJournalV1;
    const addresses = [
      signer.publicKey.toBase58(),
      ...plan.transaction.message.staticAccountKeys.slice(1).map((address) => address.toBase58()),
    ];
    const meta: TransactionMetaObservation = Object.freeze({
      signature: signed.signature, slot: '50', blockTime: null, succeeded: true, errorText: null,
      feeLamports: '5', accountAddresses: Object.freeze(addresses),
      preBalances: Object.freeze(addresses.map((_, index) => index === 0 ? '100' : '1')),
      postBalances: Object.freeze(addresses.map((_, index) => index === 0 ? '95' : '1')),
      logMessages: Object.freeze([]),
      returnData: Object.freeze({ programId: plan.report.route.claimsProgram, data: new Uint8Array([44, 55]) }),
      transactionBytes: signed.wireBytes,
    });
    const accountRows = () => [
      { address: plan.report.route.aggregate, account: material(plan.report.route.claimsProgram, 1) },
      { address: plan.report.route.position, account: material(plan.report.route.claimsProgram, 2) },
      { address: plan.report.route.custodyReplay, account: material(plan.report.route.custodyProgram, 3) },
      { address: plan.report.route.hoard, account: material(plan.report.route.tokenProgram, 4) },
      { address: plan.report.route.recipient, account: material(plan.report.route.tokenProgram, 5) },
    ];
    const client = (changedMeta: TransactionMetaObservation = meta, changedRows = accountRows()) => ({
      transaction: async () => changedMeta,
      finalizedSlot: async () => '51',
      multipleAccounts: async () => ({ slot: '51', accounts: Object.freeze(changedRows) }),
    });
    let receipt = new Uint8Array();
    await expect(finalizePayoutOperationV1(client(), journal, plan, async (_report, post) => {
      receipt = new Uint8Array(post.receiptBytes);
    })).resolves.toEqual({ signature: signed.signature, observedSlot: '51', payout: '7' });
    expect(receipt).toEqual(new Uint8Array([44, 55]));

    await expect(finalizePayoutOperationV1(client({ ...meta, transactionBytes: plan.wireBytes }), journal, plan, async () => {}))
      .rejects.toThrow(/packet differs/);
    await expect(finalizePayoutOperationV1(client({ ...meta, returnData: null }), journal, plan, async () => {}))
      .rejects.toThrow(/Claims-produced return receipt/);
    await expect(finalizePayoutOperationV1(client({
      ...meta, returnData: { programId: key(99), data: new Uint8Array([44, 55]) },
    }), journal, plan, async () => {})).rejects.toThrow(/Claims-produced return receipt/);
    await expect(finalizePayoutOperationV1(client({
      ...meta, postBalances: Object.freeze(meta.postBalances.map((value, index) => index === 1 ? '0' : value)),
    }), journal, plan, async () => {})).rejects.toThrow(/lamport balances/);
    const wrongOwner = accountRows();
    wrongOwner[0] = { ...wrongOwner[0]!, account: material(key(99), 1) };
    await expect(finalizePayoutOperationV1(client(meta, wrongOwner), journal, plan, async () => {}))
      .rejects.toThrow(/another owner/);
    const reordered = accountRows();
    reordered.reverse();
    await expect(finalizePayoutOperationV1(client(meta, reordered), journal, plan, async () => {}))
      .rejects.toThrow(/ordered account closure/);
  });
});
