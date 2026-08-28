import { PublicKey } from '@solana/web3.js';

import { hex } from './bytes';
import * as Hot from './generated/directInlineV3';
import { RESOLUTION_CERTIFICATE_BYTES_V2 } from './generated/resolutionCertificateV2';
import { inspectRationalCapabilityCommonV4 } from './rationalCapabilityChainV4';
import {
  acquireRationalHotAccountsV4,
  authenticateRationalProductBasisRecordV3,
  type RationalHotRpcV4,
} from './rationalRetireReceiptV4';
import {
  evaluateRationalTerminalPayoutV3,
  type RationalTerminalPayoutV3,
} from './rationalTerminalHotV3';
import {
  bindTerminalResolutionCertificateV2,
  decodeResolutionCertificateV2,
  type ResolutionCertificateV2,
} from './resolutionCertificateV2';

const MAX_U64 = 18_446_744_073_709_551_615n;
const RATIONAL_TERMINAL_HOT_REQUEST_SCHEMA_ID_V3 = Uint8Array.from([
  0x8b,0xab,0xcd,0x90,0x65,0xc6,0x52,0x25,0x32,0xe2,0x6c,0x60,0x63,0x61,0x56,0x72,
  0x4f,0x7f,0x6c,0xfc,0xfb,0xa2,0x60,0x82,0x75,0x86,0x27,0xc4,0x9c,0xdc,0x80,0x3b,
]);
export type RationalTerminalReadinessV4 = Readonly<{
  observedSlot: string;
  market: string;
  generation: bigint;
  actor: string;
  descriptorId: Uint8Array;
  capabilityDigest: Uint8Array;
  basisDigest: Uint8Array;
  semanticBasisId: Uint8Array;
  resultOutcomeCount: number;
  representationWidth: number;
  terminalWinner: number;
  selectedOutcome: number;
  rawQuantity: bigint;
  rawShardBurn: bigint;
  terminalCertificateAddress: string;
  terminalCertificate: ResolutionCertificateV2;
  payout: RationalTerminalPayoutV3;
  executionStatus: 'blocked';
  refusal: string;
}>;

/**
 * Read the complete immutable Product/representation terminal semantics.
 * This is an untrusted browser projection; the Rust operator remains the sole
 * emitter of SignedDeltaV3 and Custody authority/digest material.
 */
export async function inspectRationalTerminalReadinessV4(
  client: RationalHotRpcV4,
  input: Readonly<{
    payer: string;
    actor: string;
    fixedAccounts: ReadonlyArray<string>;
    lookupTable: string;
    descriptorId: string;
    selectedOutcome: number;
    rawQuantity: bigint;
  }>,
): Promise<RationalTerminalReadinessV4> {
  if (!Number.isInteger(input.selectedOutcome) || input.selectedOutcome < 0 || input.selectedOutcome > 0xffff_ffff) {
    throw new Error('selected representation outcome must be one canonical u32 index');
  }
  if (input.rawQuantity <= 0n || input.rawQuantity > MAX_U64) throw new Error('terminal quantity must be 1..u64::MAX raw units');
  const common = await inspectRationalCapabilityCommonV4(client, {
    phase: 'terminal', selector: 5, requestSchema: RATIONAL_TERMINAL_HOT_REQUEST_SCHEMA_ID_V3,
    payer: input.payer, actor: input.actor, fixedAccounts: input.fixedAccounts, lookupTable: input.lookupTable,
    descriptorId: input.descriptorId,
  });
  const admitted = await authenticateRationalProductBasisRecordV3(client, common.accounts, {
    registry: common.registry,
    rawAddress: common.fixed[Hot.HOT_LINKED_BASIS_RAW_ACCOUNT_V3]?.address ?? '',
    stagingAddress: common.fixed[Hot.HOT_LINKED_BASIS_STAGING_ACCOUNT_V3]?.address ?? '',
    productId: common.product.productId,
    domainDigest: common.domainDigest,
    domainBytes: common.domainRaw.data,
    representationWidth: common.descriptor.outcomeCount,
  });
  if (input.selectedOutcome >= admitted.basis.width) throw new Error('selected representation outcome is outside Product basis K');
  if (common.market.terminalWinner >= common.product.outcomeCount) throw new Error('Core terminal winner is outside Product result N');
  const terminalCertificateAddress = new PublicKey(common.market.terminalReceipt).toBase58();
  const observation = await acquireRationalHotAccountsV4(client, [terminalCertificateAddress], common.observedSlot);
  const terminalAccount = observation.accounts.get(terminalCertificateAddress);
  if (terminalAccount === null || terminalAccount === undefined) throw new Error('Core terminal ResolutionCertificateV2 is absent');
  if (terminalAccount.owner !== common.activation.resolution || terminalAccount.executable
      || terminalAccount.data.length !== RESOLUTION_CERTIFICATE_BYTES_V2) {
    throw new Error('Core terminal receipt is not exact Resolution-owned ResolutionCertificateV2 state');
  }
  const rent = await client.minimumBalanceForRentExemption(RESOLUTION_CERTIFICATE_BYTES_V2);
  if (BigInt(terminalAccount.lamports) < BigInt(rent.lamports)) throw new Error('ResolutionCertificateV2 is below its current exact rent minimum');
  const terminalCertificate = bindTerminalResolutionCertificateV2(
    decodeResolutionCertificateV2(terminalAccount.data),
    {
      receiptAccount: new PublicKey(terminalCertificateAddress).toBytes(),
      market: new PublicKey(common.marketAddress).toBytes(),
      sourceMaterial: common.market.resolutionPolicy,
      productRecordDigest: common.market.productRecord,
      generation: common.market.generation,
      selector: common.market.terminalWinner,
      outcomeCount: common.product.outcomeCount,
    },
  );
  const terminalCoordinate = admitted.basis.kind === 'graded-exact-complement'
      && terminalCertificate.kind === 'resolution-success'
    ? Object.freeze({ numerator: terminalCertificate.resultNumerator, denominator: terminalCertificate.resultDenominator })
    : null;
  const payout = evaluateRationalTerminalPayoutV3({
    basis: admitted.basis.bytes, resultOutcomeCount: common.product.outcomeCount,
    terminalWinner: common.market.terminalWinner, selectedOutcome: input.selectedOutcome,
    rawQuantity: input.rawQuantity, terminalCoordinate,
  });
  const rawShardBurn = common.descriptor.denominator * input.rawQuantity;
  if (rawShardBurn > MAX_U64) throw new Error('terminal raw shard burn exceeds u64::MAX');
  const refusal = payout.scenario === 'graded-rational'
    ? 'The deployed Claims terminal_settlement_v3 still derives an impossible Registry coordinate from Core terminal_receipt. This client authenticates the real ResolutionCertificateV2 and refuses submission until a Claims Upgrade consumes that certificate directly.'
    : 'The browser authenticates the canonical ResolutionCertificateV2, but has no binding to the canonical Rust SignedDeltaV3/Custody emitter. It will not reconstruct that authority or request digest in TypeScript.';
  return Object.freeze({ observedSlot: observation.slot, market: common.marketAddress, generation: common.market.generation,
    actor: common.actor, descriptorId: common.descriptorId, capabilityDigest: common.capabilitySelection.digest,
    basisDigest: admitted.digest, semanticBasisId: admitted.semanticBasisId,
    resultOutcomeCount: common.product.outcomeCount, representationWidth: admitted.basis.width,
    terminalWinner: common.market.terminalWinner, selectedOutcome: input.selectedOutcome,
    rawQuantity: input.rawQuantity, rawShardBurn, terminalCertificateAddress, terminalCertificate, payout,
    executionStatus: 'blocked',
    refusal,
  });
}

export function rationalTerminalReadinessSummaryV4(value: RationalTerminalReadinessV4): Readonly<Record<string, string>> {
  return Object.freeze({ market: value.market, descriptor: hex(value.descriptorId), basis: hex(value.basisDigest),
    widths: `K=${value.representationWidth} claims over N=${value.resultOutcomeCount} terminal results`,
    result: `${value.payout.scenario} · winner ${value.terminalWinner} · claim ${value.selectedOutcome}`,
    certificate: `${value.terminalCertificate.kind} · ${value.terminalCertificate.resultNumerator.toString()}/${value.terminalCertificate.resultDenominator.toString()}`,
    economics: `${value.rawShardBurn.toString()} shard atoms → ${value.payout.rawPayout.toString()} collateral atoms${value.payout.losing ? ' (exact zero)' : ''}` });
}
