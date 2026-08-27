import { Keypair } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { CORE_FOUND_ACCOUNT_COUNT_V2 } from '../generated/coreFound';
import {
  CLAIMS_FOUNDING_ACCOUNT_COUNT_V5,
  GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1,
  GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1,
  GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1,
  GENERIC_MARKET_FOUNDING_DISTINCT_WRITABLE_V1,
  GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V1,
  GENERIC_MARKET_FOUNDING_INSTRUCTION_BYTES_V1,
  GENERIC_MARKET_FOUNDING_MAGIC_V1,
  GENERIC_MARKET_FOUNDING_PREFIX_ACCOUNT_COUNT_V1,
  MAX_TX_ACCOUNT_LOCKS_V1,
  PROJECTED_CUSTODY_LOCK_CLOSE_ACCOUNT_COUNT_V1,
  PROJECTED_CUSTODY_REALIZE_ACCOUNT_COUNT_V1,
} from '../generated/genericFoundingV1';
import {
  CLOCK_SYSVAR_ID,
  INSTRUCTIONS_SYSVAR_ID,
  RENT_SYSVAR_ID,
  SYSTEM_PROGRAM_ID,
  buildGenericMarketFoundingFrameV1,
  genericMarketFoundingInstructionV1,
  type GenericMarketFoundingCoordinatesV1,
} from './genericMarketFounding';

/**
 * Distinct addresses so a misplaced meta is a visible disagreement.
 *
 * A frame built from repeated placeholders would pass every width check while
 * putting the Hoard where the Market belongs, so every role gets its own key
 * and the assertions below name roles rather than indices wherever they can.
 */
function keys(): GenericMarketFoundingCoordinatesV1 {
  const next = () => Keypair.generate().publicKey.toBase58();
  const market = next();
  const foundAuthority = next();
  const foundSnapshotKeys = Array.from({ length: CORE_FOUND_ACCOUNT_COUNT_V2 }, (_, index) => {
    if (index === 0) return foundAuthority;
    if (index === 1) return market;
    return next();
  });
  return Object.freeze({
    foundRequest: next(),
    lockRequest: next(),
    realizeRequest: next(),
    claimsRequest: next(),
    tradingProgram: next(),
    tradingProgramData: next(),
    coreProgram: next(),
    coreProgramData: next(),
    claimsProgram: next(),
    claimsProgramData: next(),
    custodyProgram: next(),
    custodyProgramData: next(),
    registryProgram: next(),
    rentProgram: next(),
    tokenProgram: next(),
    activationCache: next(),
    lockCaller: next(),
    realizeCaller: next(),
    claimsCaller: next(),
    foundAuthority,
    openAuthority: next(),
    market,
    credit: next(),
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
    productRaw: next(),
    productStaging: next(),
    resultDomainRaw: next(),
    resultDomainStaging: next(),
    portfolioRaw: next(),
    portfolioStaging: next(),
    foundSnapshotKeys,
    fundingStates: [next(), next(), next()],
  });
}

describe('the DCLTGMF1 outer frame', () => {
  it('carries exactly the eight-byte discriminator and no payload', () => {
    const frame = buildGenericMarketFoundingFrameV1(keys());
    expect(frame.data.length).toBe(GENERIC_MARKET_FOUNDING_INSTRUCTION_BYTES_V1);
    expect(new TextDecoder().decode(frame.data)).toBe(GENERIC_MARKET_FOUNDING_MAGIC_V1);
    // The route matches by equality, not by prefix, so a ninth byte is a
    // different instruction rather than a tolerated one.
    expect(genericMarketFoundingInstructionV1(frame).data.length).toBe(8);
  });

  it('sums its six stage widths to the total the reference client pins', () => {
    // 135 is transcribed in `market.rs` as one number. If a stage width moves
    // and that number does not, this is where the two disagree.
    const stages = GENERIC_MARKET_FOUNDING_PREFIX_ACCOUNT_COUNT_V1
      + PROJECTED_CUSTODY_LOCK_CLOSE_ACCOUNT_COUNT_V1
      + GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1
      + GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1
      + PROJECTED_CUSTODY_REALIZE_ACCOUNT_COUNT_V1
      + CLAIMS_FOUNDING_ACCOUNT_COUNT_V5
      + GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1;
    expect(stages).toBe(GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V1);
  });

  it('lays the stages out at the offsets the route parses them from', () => {
    const frame = buildGenericMarketFoundingFrameV1(keys());
    expect(frame.accounts.length).toBe(GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V1 + 3);
    const lockStart = GENERIC_MARKET_FOUNDING_PREFIX_ACCOUNT_COUNT_V1;
    const foundStart = lockStart + PROJECTED_CUSTODY_LOCK_CLOSE_ACCOUNT_COUNT_V1;
    const foundCount = GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1 + 3 + GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1;
    const realizeStart = foundStart + foundCount;
    const claimsStart = realizeStart + PROJECTED_CUSTODY_REALIZE_ACCOUNT_COUNT_V1;
    const openStart = claimsStart + CLAIMS_FOUNDING_ACCOUNT_COUNT_V5;
    expect(frame.stageBounds).toEqual({
      prefix: { start: 0, count: GENERIC_MARKET_FOUNDING_PREFIX_ACCOUNT_COUNT_V1 },
      lock: { start: lockStart, count: PROJECTED_CUSTODY_LOCK_CLOSE_ACCOUNT_COUNT_V1 },
      found: { start: foundStart, count: foundCount },
      realize: { start: realizeStart, count: PROJECTED_CUSTODY_REALIZE_ACCOUNT_COUNT_V1 },
      claims: { start: claimsStart, count: CLAIMS_FOUNDING_ACCOUNT_COUNT_V5 },
      open: { start: openStart, count: GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1 },
    });
    expect(openStart + GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1).toBe(frame.accounts.length);
  });

  it('puts the four requests and the instructions sysvar in the prefix, in order', () => {
    const input = keys();
    const frame = buildGenericMarketFoundingFrameV1(input);
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

  it('names exactly the eleven writable keys, and they are the accounts the founding mutates', () => {
    const input = keys();
    const frame = buildGenericMarketFoundingFrameV1(input);
    expect(frame.distinctWritable.length).toBe(GENERIC_MARKET_FOUNDING_DISTINCT_WRITABLE_V1);
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
    ].sort());
  });

  it('unions writability so one key never carries two privileges', () => {
    const input = keys();
    const frame = buildGenericMarketFoundingFrameV1(input);
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
    const frame = buildGenericMarketFoundingFrameV1(keys());
    expect(frame.accounts.some((account) => account.signer)).toBe(false);
  });

  it('stays inside the transaction account-lock limit with the fee payer counted', () => {
    const frame = buildGenericMarketFoundingFrameV1(keys());
    expect(frame.distinctAccounts.length + 1).toBeLessThanOrEqual(MAX_TX_ACCOUNT_LOCKS_V1);
  });

  it('widens by exactly one account per FundingState', () => {
    const base = keys();
    for (const count of [1, 3, 16]) {
      const frame = buildGenericMarketFoundingFrameV1({
        ...base,
        fundingStates: Array.from({ length: count }, () => Keypair.generate().publicKey.toBase58()),
      });
      expect(frame.accounts.length).toBe(GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V1 + count);
      expect(frame.fundingCount).toBe(count);
    }
  });

  it('places the three runtime accounts only where the ABI names them', () => {
    const frame = buildGenericMarketFoundingFrameV1(keys());
    const at = (address: string) => frame.accounts.flatMap((account) => (account.address === address ? [account.stage] : []));
    expect(at(INSTRUCTIONS_SYSVAR_ID)).toEqual(['prefix']);
    expect(at(SYSTEM_PROGRAM_ID)).toEqual(['claims']);
    expect(at(CLOCK_SYSVAR_ID)).toEqual(['found', 'open']);
    expect(at(RENT_SYSVAR_ID)).toEqual(['claims', 'open']);
  });
});

describe('the DCLTGMF1 outer frame refuses', () => {
  it('a FundingState tail outside 1..16', () => {
    const base = keys();
    expect(() => buildGenericMarketFoundingFrameV1({ ...base, fundingStates: [] })).toThrow(/1\.\.16/);
    expect(() => buildGenericMarketFoundingFrameV1({
      ...base,
      fundingStates: Array.from({ length: 17 }, () => Keypair.generate().publicKey.toBase58()),
    })).toThrow(/1\.\.16/);
  });

  it('a Found31 projection of the wrong width', () => {
    const base = keys();
    expect(() => buildGenericMarketFoundingFrameV1({ ...base, foundSnapshotKeys: base.foundSnapshotKeys.slice(0, 30) })).toThrow(/exactly 31 Found31 keys/);
  });

  it('a frame whose writable set is not the eleven the outer requires', () => {
    // Aliasing the Hoard onto the readonly Custody authority removes one
    // distinct writable key without changing the frame's width, which is
    // exactly the class of slip the client's own assertion exists to catch.
    const base = keys();
    expect(() => buildGenericMarketFoundingFrameV1({ ...base, hoardVault: base.sourceVault })).toThrow(/declared 10 writable keys, not the 11/);
  });

  it('noncanonical base58 in any coordinate', () => {
    const base = keys();
    expect(() => buildGenericMarketFoundingFrameV1({ ...base, founder: 'not-a-key' })).toThrow();
  });
});
