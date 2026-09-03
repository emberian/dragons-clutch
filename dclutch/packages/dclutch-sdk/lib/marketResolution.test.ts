import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import * as Abi from '@dclutch/sdk/generated/resolutionCertificateV2';
import { type DecodedMarketDiscoveryCardV1 } from './marketDiscovery';
import {
  exactRatioDecimalV1,
  inspectMarketResolutionV1,
  marketRedemptionStateV1,
  terminalCertificateAddressV1,
} from './marketResolution';
import { type RpcAccount } from './rpc';

/**
 * The certificate reader, held to the two things it is for: it must FOLLOW the
 * Market's terminal-receipt slot to a real account, and it must refuse rather
 * than narrate whenever the account it finds is not one this Market's own
 * terminal authority vouches for.
 *
 * Every refusal below is proved by mutating ONE field of a certificate that
 * otherwise authenticates, so a case that goes green because the whole fixture
 * was wrong is not available.
 */

const RESOLUTION = 'ResoLut1on111111111111111111111111111111111';
const MARKET = '11111111111111111111111111111112';

function u32(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).setUint32(offset, value, true);
}
function u64(bytes: Uint8Array, offset: number, value: bigint): void {
  new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).setBigUint64(offset, value, true);
}
function i128(bytes: Uint8Array, offset: number, value: bigint): void {
  let remaining = value < 0n ? (1n << 128n) + value : value;
  for (let index = 0; index < 16; index += 1) {
    bytes[offset + index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
}
function fill(bytes: Uint8Array, offset: number, byte: number): void {
  for (let index = 0; index < 32; index += 1) bytes[offset + index] = byte;
}

/** A canonical success certificate for `market`, receipted at `receipt`. */
function successCertificate(receipt: string, selector: number): Uint8Array {
  const bytes = new Uint8Array(Abi.RESOLUTION_CERTIFICATE_BYTES_V2);
  bytes.set(new TextEncoder().encode(Abi.RESOLUTION_CERTIFICATE_MAGIC_V2), Abi.CERTIFICATE_V2_MAGIC_OFFSET);
  new DataView(bytes.buffer).setUint16(Abi.CERTIFICATE_V2_VERSION_OFFSET, Abi.RESOLUTION_CERTIFICATE_VERSION_V2, true);
  bytes[Abi.CERTIFICATE_V2_KIND_OFFSET] = Abi.RESOLUTION_CERTIFICATE_SUCCESS_KIND_V2;
  bytes.set(new PublicKey(MARKET).toBytes(), Abi.CERTIFICATE_V2_MARKET_OFFSET);
  fill(bytes, Abi.CERTIFICATE_V2_ROUTE_OFFSET, 0x11);
  fill(bytes, Abi.CERTIFICATE_V2_SOURCE_MATERIAL_OFFSET, 0x22);
  fill(bytes, Abi.CERTIFICATE_V2_PRODUCT_RECORD_OFFSET, 0x33);
  fill(bytes, Abi.CERTIFICATE_V2_PROVIDER_EVIDENCE_OFFSET, 0x44);
  bytes.set(new PublicKey(receipt).toBytes(), Abi.CERTIFICATE_V2_RECEIPT_ACCOUNT_OFFSET);
  u64(bytes, Abi.CERTIFICATE_V2_GENERATION_OFFSET, 7n);
  u32(bytes, Abi.CERTIFICATE_V2_ATTEMPT_INDEX_OFFSET, 0);
  u32(bytes, Abi.CERTIFICATE_V2_SCHEDULE_INDEX_OFFSET, 1);
  u32(bytes, Abi.CERTIFICATE_V2_SELECTOR_OFFSET, selector);
  u64(bytes, Abi.CERTIFICATE_V2_WORK_PAID_OFFSET, 500n);
  u64(bytes, Abi.CERTIFICATE_V2_FUNDING_REMAINING_OFFSET, 0n);
  i128(bytes, Abi.CERTIFICATE_V2_RESULT_NUMERATOR_OFFSET, 10_062_091_764n);
  u64(bytes, Abi.CERTIFICATE_V2_RESULT_DENOMINATOR_OFFSET, 100_000_000n);
  u64(bytes, Abi.CERTIFICATE_V2_OBSERVED_AT_OFFSET, 1_788_415_399n);
  return bytes;
}

function terminalCard(receiptId: string, winner: number, hoardAtoms: string): DecodedMarketDiscoveryCardV1 {
  return {
    status: 'decoded', address: MARKET,
    provenance: { kind: 'chain', observedSlot: '900' }, observedSlot: '900',
    phase: 'Terminal', readiness: 'Consumed', generation: '7',
    outstandingCapabilities: '0', principalCapSets: '0',
    settlement: { status: 'terminal', label: 'terminal receipt accepted', winner, receiptId },
    identity: {
      schemaMagic: 'DCLTCOR3', schemaVersion: 3, accountBytes: 368,
      marketId: '00'.repeat(32), realmId: '01'.repeat(32), productRecordId: '33'.repeat(32),
      productInstanceId: '03'.repeat(32), resolutionPolicyId: '04'.repeat(32),
      capabilityManifestId: '05'.repeat(32), selectedReleaseSetId: '06'.repeat(32),
      registryProgram: MARKET, rentBeneficiary: MARKET,
    },
    collateral: { status: 'unread', realmContentId: '01'.repeat(32), reason: 'not needed here' },
    liability: {
      status: 'bound', observedSlot: '900', aggregateAddress: MARKET, claimsProgramId: MARKET,
      claimCount: 4, revision: '3', generation: '7', liabilityBasisId: '07'.repeat(32),
      custodyContext: '08'.repeat(32),
      supplyAtoms: ['500000000', '500000000', '500000000', '500000000'],
      requiredBackingAtoms: '500000000', requiredBackingBasis: 'winning-claim-supply',
    },
    hoard: {
      status: 'derived', observedSlot: '900', address: MARKET, custodyProgramId: MARKET,
      custodyContext: '08'.repeat(32), custodyAuthority: MARKET, collateralMint: MARKET,
      tokenProgram: MARKET, principalAtoms: hoardAtoms, mintDisplayDecimals: 6,
    },
    capabilities: { status: 'unread', manifestId: '05'.repeat(32), reason: 'not needed here' },
    bindings: [], refusal: null,
  } as DecodedMarketDiscoveryCardV1;
}

function clientReturning(account: RpcAccount | null) {
  return { accountInfo: async () => ({ slot: '900', account }) };
}

function certificateAccount(data: Uint8Array, owner = RESOLUTION): RpcAccount {
  return { data, executable: false, lamports: '2786520', owner, space: data.length };
}

const RECEIPT = 'CertiF1cate11111111111111111111111111111111';
const RECEIPT_ID = Buffer.from(new PublicKey(RECEIPT).toBytes()).toString('hex');

describe('the terminal certificate a Market names', () => {
  it('reads the receipt slot as an ADDRESS, which is what those 32 bytes are', () => {
    expect(terminalCertificateAddressV1(RECEIPT_ID)).toBe(RECEIPT);
    // The explorer files the same bytes as a content digest with no account
    // behind them, which is true of the record layer and is why nothing was
    // following them. A digest-shaped string that is not 32 bytes still refuses.
    expect(() => terminalCertificateAddressV1('cafe')).toThrow('32 bytes of hex');
  });

  it('authenticates a success certificate and reports the observation exactly', async () => {
    const resolution = await inspectMarketResolutionV1(
      clientReturning(certificateAccount(successCertificate(RECEIPT, 1))),
      { card: terminalCard(RECEIPT_ID, 1, '0'), resolutionProgramId: RESOLUTION, floorSlot: '900' },
    );
    expect(resolution.status, resolution.status === 'refused' ? resolution.reason : '').toBe('authenticated');
    if (resolution.status !== 'authenticated') return;
    expect(resolution.kind).toBe('resolution-success');
    expect(resolution.sourceReported).toBe(true);
    expect(resolution.selector).toBe(1);
    // The ratio is carried as two integers and rendered by exact long division;
    // a float would print 100.62091763999999 for this pair.
    expect(resolution.observation?.decimal).toBe('100.62091764');
    expect(resolution.observation?.numerator).toBe(10_062_091_764n);
    expect(resolution.observation?.atUnixSeconds).toBe(1_788_415_399n);
    // With no window handed over, the standing is `unwindowed` -- which says
    // this reader was not given one, never that the market has none.
    expect(resolution.observation?.standing).toBe('unwindowed');
    expect(resolution.providerEvidenceId).toBe('44'.repeat(32));
  });

  it('places the observation inside, before or after the window it is given', async () => {
    const window = (start: bigint, end: bigint) => ({
      address: MARKET, observedSlot: '900',
      productRecord: MARKET, sourceMaterialRecord: MARKET, resultDomainRecord: MARKET,
      portfolioRecord: MARKET, windowSpecRecord: MARKET,
      cutDenominator: 100n, cuts: [9900n, 10300n], regionCount: 3, outcomeCount: 4,
      window: { startUnixSeconds: start, endUnixSeconds: end }, windowRefusal: null,
    });
    const read = async (start: bigint, end: bigint) => inspectMarketResolutionV1(
      clientReturning(certificateAccount(successCertificate(RECEIPT, 1))),
      { card: terminalCard(RECEIPT_ID, 1, '0'), resolutionProgramId: RESOLUTION, floorSlot: '900', question: window(start, end) },
    );
    const inside = await read(1_788_415_296n, 1_788_417_096n);
    const before = await read(1_788_415_400n, 1_788_417_096n);
    const after = await read(1_788_000_000n, 1_788_415_398n);
    expect(inside.status === 'authenticated' && inside.observation?.standing).toBe('inside');
    expect(before.status === 'authenticated' && before.observation?.standing).toBe('before');
    expect(after.status === 'authenticated' && after.observation?.standing).toBe('after');
  });

  it('refuses a certificate account owned by anything but Resolution', async () => {
    const resolution = await inspectMarketResolutionV1(
      clientReturning(certificateAccount(successCertificate(RECEIPT, 1), MARKET)),
      { card: terminalCard(RECEIPT_ID, 1, '0'), resolutionProgramId: RESOLUTION, floorSlot: '900' },
    );
    expect(resolution.status).toBe('refused');
    expect(resolution.status === 'refused' && resolution.reason).toContain('owned by');
  });

  it('refuses a certificate whose selector is not the winner Core recorded', async () => {
    // The one field moved: the certificate says 1, the Market says 2. This is
    // the join, and without it a page would print a certificate belonging to
    // another answer beside this market's own winner and nothing would notice.
    const resolution = await inspectMarketResolutionV1(
      clientReturning(certificateAccount(successCertificate(RECEIPT, 1))),
      { card: terminalCard(RECEIPT_ID, 2, '0'), resolutionProgramId: RESOLUTION, floorSlot: '900' },
    );
    expect(resolution.status).toBe('refused');
    expect(resolution.status === 'refused' && resolution.reason).toContain('terminal authority');
  });

  it('refuses a success certificate that claims the source-failure cell', async () => {
    // Selector 3 of 4 is the failure cell, and a SUCCESS certificate may not
    // carry it — that is the one index the certificate itself pins.
    const resolution = await inspectMarketResolutionV1(
      clientReturning(certificateAccount(successCertificate(RECEIPT, 3))),
      { card: terminalCard(RECEIPT_ID, 3, '0'), resolutionProgramId: RESOLUTION, floorSlot: '900' },
    );
    expect(resolution.status).toBe('refused');
    expect(resolution.status === 'refused' && resolution.reason).toContain('terminal authority');
  });

  it('says "not yet" for an open market rather than refusing', async () => {
    const open = { ...terminalCard(RECEIPT_ID, 1, '500000000'), phase: 'Open', settlement: { status: 'open', label: 'no terminal receipt' } } as DecodedMarketDiscoveryCardV1;
    const resolution = await inspectMarketResolutionV1(
      clientReturning(null),
      { card: open, resolutionProgramId: RESOLUTION, floorSlot: '900' },
    );
    // "Not yet" and "it did not read" are different facts, and a page that
    // conflates them tells a reader to wait for something that already failed.
    expect(resolution.status).toBe('not-terminal');
  });

  it('reports a missing certificate account as a refusal with its own reason', async () => {
    const resolution = await inspectMarketResolutionV1(
      clientReturning(null),
      { card: terminalCard(RECEIPT_ID, 1, '0'), resolutionProgramId: RESOLUTION, floorSlot: '900' },
    );
    expect(resolution.status).toBe('refused');
    expect(resolution.status === 'refused' && resolution.reason).toContain('does not exist');
  });
});

describe('exact ratio rendering', () => {
  it('divides exactly or answers null, never a rounded string', () => {
    expect(exactRatioDecimalV1(10_062_091_764n, 100_000_000n)).toBe('100.62091764');
    expect(exactRatioDecimalV1(200n, 1n)).toBe('200');
    expect(exactRatioDecimalV1(-5n, 2n)).toBe('-2.5');
    // A third does not terminate, so the caller shows the pair instead.
    expect(exactRatioDecimalV1(1n, 3n)).toBeNull();
    expect(exactRatioDecimalV1(1n, 0n)).toBeNull();
  });
});

describe('what is left in the vault against what the winners are owed', () => {
  it('reads none, partial and complete, and never a negative redemption', () => {
    expect(marketRedemptionStateV1(terminalCard(RECEIPT_ID, 1, '500000000'))).toMatchObject({ status: 'read', redeemedAtoms: '0', progress: 'none' });
    expect(marketRedemptionStateV1(terminalCard(RECEIPT_ID, 1, '200000000'))).toMatchObject({ status: 'read', redeemedAtoms: '300000000', progress: 'partial' });
    expect(marketRedemptionStateV1(terminalCard(RECEIPT_ID, 1, '0'))).toMatchObject({ status: 'read', redeemedAtoms: '500000000', progress: 'complete' });
    // Over-collateralised is not a negative redemption; it is no redemption.
    expect(marketRedemptionStateV1(terminalCard(RECEIPT_ID, 1, '900000000'))).toMatchObject({ status: 'read', redeemedAtoms: '0', progress: 'none' });
  });

  it('says nothing at all about a market that has no answer', () => {
    const open = { ...terminalCard(RECEIPT_ID, 1, '500000000'), settlement: { status: 'open', label: 'no terminal receipt' } } as DecodedMarketDiscoveryCardV1;
    expect(marketRedemptionStateV1(open)).toMatchObject({ status: 'unread' });
  });
});
