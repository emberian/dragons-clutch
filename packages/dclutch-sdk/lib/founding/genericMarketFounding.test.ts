import {
  AddressLookupTableAccount,
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
} from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  CLAIMS_FOUNDING_ACCOUNT_COUNT_V5,
  GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1,
  GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1,
  GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1,
  GENERIC_MARKET_FOUNDING_DISTINCT_WRITABLE_V3,
  GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V3,
  GENERIC_MARKET_FOUNDING_INSTRUCTION_BYTES_V3,
  GENERIC_MARKET_FOUNDING_MAGIC_V3,
  GENERIC_MARKET_FOUNDING_PREFIX_ACCOUNT_COUNT_V3,
  MAX_TX_ACCOUNT_LOCKS_V2,
  PROJECTED_FOUND_ACCOUNT_COUNT_V2,
  PROJECTED_CUSTODY_LOCK_CLOSE_ACCOUNT_COUNT_V1,
  PROJECTED_CUSTODY_REALIZE_ACCOUNT_COUNT_V1,
} from '../generated/genericFoundingV1';
import {
  INSTRUCTIONS_SYSVAR_ID,
  SYSTEM_PROGRAM_ID,
  buildGenericMarketFoundingFrameV3,
  genericMarketFoundingInstructionV3,
  type GenericMarketFoundingCoordinatesV3,
} from './genericMarketFounding';
import { canonicalLookupAddressesV1 } from './lookupTable';

const DIRECT_FUNDING_MASKS_V2 = Object.freeze([0b0111, 0b1000] as const);

/**
 * Distinct addresses so a misplaced meta is a visible disagreement.
 *
 * A frame built from repeated placeholders would pass every width check while
 * putting the Hoard where the Market belongs, so every role gets its own key
 * and the assertions below name roles rather than indices wherever they can.
 */
function keys(): GenericMarketFoundingCoordinatesV3 {
  const next = () => Keypair.generate().publicKey.toBase58();
  const market = next();
  const foundAuthority = next();
  const credit = next();
  const rentProgram = next();
  const productRaw = next();
  const productStaging = next();
  const resultDomainRaw = next();
  const resultDomainStaging = next();
  const portfolioRaw = next();
  const portfolioStaging = next();
  const manifestRaw = next();
  const manifestStaging = next();
  const activationCache = next();
  const coreProgram = next();
  const coreProgramData = next();
  const registryProgram = next();
  const projectedFoundKeys = [
    foundAuthority,
    market,
    credit,
    rentProgram,
    productRaw,
    productStaging,
    resultDomainRaw,
    resultDomainStaging,
    portfolioRaw,
    portfolioStaging,
    manifestRaw,
    manifestStaging,
    activationCache,
    coreProgram,
    coreProgramData,
    registryProgram,
    SYSTEM_PROGRAM_ID,
    next(),
    next(),
    next(),
    next(),
    next(),
    next(),
    next(),
  ];
  expect(projectedFoundKeys.length).toBe(PROJECTED_FOUND_ACCOUNT_COUNT_V2);
  return Object.freeze({
    foundRequest: next(),
    lockRequest: next(),
    realizeRequest: next(),
    claimsRequest: next(),
    tradingProgram: next(),
    tradingProgramData: next(),
    coreProgram,
    coreProgramData,
    claimsProgram: next(),
    claimsProgramData: next(),
    custodyProgram: next(),
    custodyProgramData: next(),
    registryProgram,
    rentProgram,
    tokenProgram: next(),
    activationCache,
    lockCaller: next(),
    realizeCaller: next(),
    claimsCaller: next(),
    foundAuthority,
    openAuthority: next(),
    callerBumps: [251, 252, 253, 254, 255] as const,
    market,
    credit,
    permit: next(),
    projectedReplay: next(),
    sourceReplay: next(),
    hoardVault: next(),
    sourceVault: next(),
    custodyAuthority: next(),
    collateralMint: next(),
    founder: next(),
    aggregate: next(),
    position: next(),
    admission: next(),
    liabilityBasisRaw: next(),
    liabilityBasisStaging: next(),
    productRaw,
    productStaging,
    resultDomainRaw,
    resultDomainStaging,
    portfolioRaw,
    portfolioStaging,
    projectedFoundKeys,
    fundingLedgers: [next(), next()],
    controllerFundingCheckpoint: next(),
  });
}

function compiledMessageStats(
  input: GenericMarketFoundingCoordinatesV3,
  syntheticAccounts = 0,
): Readonly<{ locks: number; bytes: number }> {
  const frame = buildGenericMarketFoundingFrameV3(input);
  const founding = genericMarketFoundingInstructionV3(frame);
  const extras = Array.from({ length: syntheticAccounts }, () => ({
    pubkey: Keypair.generate().publicKey,
    isSigner: false,
    isWritable: false,
  }));
  const detector = new TransactionInstruction({
    programId: new PublicKey(SYSTEM_PROGRAM_ID),
    keys: extras,
    data: Buffer.alloc(0),
  });
  const payer = Keypair.generate().publicKey;
  const routed = [founding, detector].flatMap((instruction) => [
    instruction.programId,
    ...instruction.keys.filter((meta) => !meta.isSigner).map((meta) => meta.pubkey),
  ]).filter((key) => !key.equals(payer));
  const canonical = canonicalLookupAddressesV1([
    ...new Set(routed.map((key) => key.toBase58())),
  ]).map((address) => new PublicKey(address));
  const table = new AddressLookupTableAccount({
    key: Keypair.generate().publicKey,
    state: {
      deactivationSlot: 0xffff_ffff_ffff_ffffn,
      lastExtendedSlot: 1,
      lastExtendedSlotStartIndex: 0,
      authority: undefined,
      addresses: canonical,
    },
  });
  const message = new TransactionMessage({
    payerKey: payer,
    recentBlockhash: Keypair.generate().publicKey.toBase58(),
    instructions: [
      ComputeBudgetProgram.requestHeapFrame({ bytes: 256 * 1024 }),
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 50_000 }),
      founding,
      detector,
    ],
  }).compileToV0Message([table]);
  const locks = message.staticAccountKeys.length + message.addressTableLookups.reduce(
    (count, lookup) => count + lookup.writableIndexes.length + lookup.readonlyIndexes.length,
    0,
  );
  return Object.freeze({ locks, bytes: message.serialize().length });
}

describe('the DCLTGMF3 outer frame', () => {
  it('carries the discriminator and five ordered caller bumps exactly', () => {
    const frame = buildGenericMarketFoundingFrameV3(keys());
    expect(frame.data.length).toBe(GENERIC_MARKET_FOUNDING_INSTRUCTION_BYTES_V3);
    expect(new TextDecoder().decode(frame.data.slice(0, 8))).toBe(GENERIC_MARKET_FOUNDING_MAGIC_V3);
    expect([...frame.data.slice(8)]).toEqual([251, 252, 253, 254, 255]);
    expect(genericMarketFoundingInstructionV3(frame).data.length).toBe(13);
  });

  it('sums its six stage widths to the total the reference client pins', () => {
    // 129 is transcribed in `market.rs` as one number. If a stage width moves
    // and that number does not, this is where the two disagree.
    const stages = GENERIC_MARKET_FOUNDING_PREFIX_ACCOUNT_COUNT_V3
      + PROJECTED_CUSTODY_LOCK_CLOSE_ACCOUNT_COUNT_V1
      + GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1
      + GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1
      + PROJECTED_CUSTODY_REALIZE_ACCOUNT_COUNT_V1
      + CLAIMS_FOUNDING_ACCOUNT_COUNT_V5
      + GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1
      + 1;
    expect(stages).toBe(GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V3);
  });

  it('lays the stages out at the offsets the route parses them from', () => {
    const frame = buildGenericMarketFoundingFrameV3(keys());
    expect(frame.accounts.length).toBe(GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V3 + 2);
    const lockStart = GENERIC_MARKET_FOUNDING_PREFIX_ACCOUNT_COUNT_V3;
    const foundStart = lockStart + PROJECTED_CUSTODY_LOCK_CLOSE_ACCOUNT_COUNT_V1;
    const foundCount = GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1 + 2 + GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1;
    const realizeStart = foundStart + foundCount;
    const claimsStart = realizeStart + PROJECTED_CUSTODY_REALIZE_ACCOUNT_COUNT_V1;
    const openStart = claimsStart + CLAIMS_FOUNDING_ACCOUNT_COUNT_V5;
    const checkpointStart = openStart + GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1;
    expect(frame.stageBounds).toEqual({
      prefix: { start: 0, count: GENERIC_MARKET_FOUNDING_PREFIX_ACCOUNT_COUNT_V3 },
      lock: { start: lockStart, count: PROJECTED_CUSTODY_LOCK_CLOSE_ACCOUNT_COUNT_V1 },
      found: { start: foundStart, count: foundCount },
      realize: { start: realizeStart, count: PROJECTED_CUSTODY_REALIZE_ACCOUNT_COUNT_V1 },
      claims: { start: claimsStart, count: CLAIMS_FOUNDING_ACCOUNT_COUNT_V5 },
      open: { start: openStart, count: GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1 },
      checkpoint: { start: checkpointStart, count: 1 },
    });
    expect(checkpointStart + 1).toBe(frame.accounts.length);
  });

  it('puts the four requests and the instructions sysvar in the prefix, in order', () => {
    const input = keys();
    const frame = buildGenericMarketFoundingFrameV3(input);
    expect(frame.accounts.slice(0, 5).map((account) => account.address)).toEqual([
      input.foundRequest,
      input.lockRequest,
      input.realizeRequest,
      input.claimsRequest,
      INSTRUCTIONS_SYSVAR_ID,
    ]);
    // All four are readonly: the route reads their bytes and never writes them.
    expect(frame.accounts.slice(0, 5).every((account) => !account.writable)).toBe(true);
  });

  it('names exactly the twelve writable keys, and they are the accounts the founding mutates', () => {
    const input = keys();
    const frame = buildGenericMarketFoundingFrameV3(input);
    expect(frame.distinctWritable.length).toBe(GENERIC_MARKET_FOUNDING_DISTINCT_WRITABLE_V3);
    expect([...frame.distinctWritable].sort()).toEqual([
      input.foundAuthority,
      input.market,
      input.credit,
      input.permit,
      input.projectedReplay,
      input.sourceReplay,
      input.hoardVault,
      input.sourceVault,
      input.aggregate,
      input.position,
      input.admission,
      input.controllerFundingCheckpoint,
    ].sort());
  });

  it('unions writability so one key never carries two privileges', () => {
    const input = keys();
    const frame = buildGenericMarketFoundingFrameV3(input);
    // The Market is readonly in the Lock, Realize and Claims stages as pushed
    // and writable in Found and Open. Solana grants per key for the whole
    // message, so every copy must agree after the union pass.
    const marketCopies = frame.accounts.filter((account) => account.address === input.market);
    expect(marketCopies.length).toBeGreaterThan(1);
    expect(marketCopies.every((account) => account.writable)).toBe(true);
    for (const address of frame.distinctAccounts) {
      const copies = frame.accounts.filter((account) => account.address === address);
      expect(new Set(copies.map((account) => account.writable)).size).toBe(1);
    }
  });

  it('declares no transaction-level signer; every stage signer is a PDA the outer signs for', () => {
    const frame = buildGenericMarketFoundingFrameV3(keys());
    expect(frame.accounts.some((account) => account.signer)).toBe(false);
  });

  it('compiles the four-entry Direct frame to 58 locks and detects the devnet 64/65 boundary', () => {
    expect(DIRECT_FUNDING_MASKS_V2[0] | DIRECT_FUNDING_MASKS_V2[1]).toBe(0b1111);
    expect(DIRECT_FUNDING_MASKS_V2[0] & DIRECT_FUNDING_MASKS_V2[1]).toBe(0);
    const direct = keys();
    expect(direct.fundingLedgers.length).toBe(2);
    const base = compiledMessageStats(direct);
    const admitted = compiledMessageStats(direct, 6);
    const refused = compiledMessageStats(direct, 7);
    expect(base.locks).toBe(58);
    expect(base.bytes).toBe(429);
    expect(admitted.bytes).toBe(441);
    expect(refused.bytes).toBe(443);
    expect(admitted.locks).toBe(MAX_TX_ACCOUNT_LOCKS_V2);
    expect(refused.locks).toBe(MAX_TX_ACCOUNT_LOCKS_V2 + 1);
    expect(admitted.locks <= MAX_TX_ACCOUNT_LOCKS_V2).toBe(true);
    expect(refused.locks <= MAX_TX_ACCOUNT_LOCKS_V2).toBe(false);
  });

  it('widens by exactly one account per controller-subset FundingLedgerV2', () => {
    const base = keys();
    for (const count of [1, 2]) {
      const frame = buildGenericMarketFoundingFrameV3({
        ...base,
        fundingLedgers: Array.from({ length: count }, () => Keypair.generate().publicKey.toBase58()),
      });
      expect(frame.accounts.length).toBe(GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V3 + count);
      expect(frame.fundingCount).toBe(count);
    }
  });

  it('keeps only the instructions sysvar and System program in the outer frame', () => {
    const frame = buildGenericMarketFoundingFrameV3(keys());
    const at = (address: string) => frame.accounts.flatMap((account) => (account.address === address ? [account.stage] : []));
    expect(at(INSTRUCTIONS_SYSVAR_ID)).toEqual(['prefix']);
    expect(at(SYSTEM_PROGRAM_ID)).toEqual(['found', 'claims']);
    expect(frame.accounts.filter((account) => account.role.endsWith('sysvar')).map((account) => account.address)).toEqual([INSTRUCTIONS_SYSVAR_ID]);
  });
});

describe('the DCLTGMF3 outer frame refuses', () => {
  it('a FundingLedgerV2 tail outside the one-or-two-controller profile', () => {
    const base = keys();
    expect(() => buildGenericMarketFoundingFrameV3({ ...base, fundingLedgers: [] })).toThrow(/one or two/);
    expect(() => buildGenericMarketFoundingFrameV3({
      ...base,
      fundingLedgers: Array.from({ length: 3 }, () => Keypair.generate().publicKey.toBase58()),
    })).toThrow(/one or two/);
  });

  it('a compact ProjectedFound V2 projection of the wrong width', () => {
    const base = keys();
    expect(() => buildGenericMarketFoundingFrameV3({ ...base, projectedFoundKeys: base.projectedFoundKeys.slice(0, 23) })).toThrow(/exactly 24 compact ProjectedFound V2 keys/);
  });

  it('a frame whose writable set is not the twelve the outer requires', () => {
    // Aliasing the Hoard onto the readonly Custody authority removes one
    // distinct writable key without changing the frame's width, which is
    // exactly the class of slip the client's own assertion exists to catch.
    const base = keys();
    expect(() => buildGenericMarketFoundingFrameV3({ ...base, hoardVault: base.sourceVault })).toThrow(/declared 11 writable keys, not the 12/);
  });

  it('noncanonical base58 in any coordinate', () => {
    const base = keys();
    expect(() => buildGenericMarketFoundingFrameV3({ ...base, founder: 'not-a-key' })).toThrow();
    expect(() => buildGenericMarketFoundingFrameV3({ ...base, callerBumps: [1, 2, 3, 4, 256] })).toThrow(/five canonical u8/);
  });
});
