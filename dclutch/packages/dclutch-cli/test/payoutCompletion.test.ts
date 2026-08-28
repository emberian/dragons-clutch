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
  authenticateCompletedCampaignEvidenceV1,
  finalizePayoutOperationV1,
  loadPayoutOperationJournalV1,
  markPayoutOperationSubmittedV1,
  parseWalletTerminalPayoutPlanInputV1,
  restorePayoutOperationJournalV1,
  signPayoutPlanV1,
  transactionSignatureV1,
  writeUnsignedPayoutOperationJournalV1,
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
