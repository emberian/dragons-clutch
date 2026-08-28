/**
 * The DCLTGMF2 outer instruction: Lock → Found → Realize → Claims → Open.
 *
 * Five stages in one rollback domain. The Market is created by the Found stage
 * and Opened by the last, so this single transaction is the whole distance
 * between a projected-Custody prestate and a live Market. A late refusal rolls
 * all five back, which is why `execute_generic_market_founding` can prove a
 * substituted Claims request costs exactly one transaction fee.
 *
 * The instruction data is eight ASCII bytes. Everything else this builder does
 * is assemble the account frame, and the frame *is* the wire: `GenericFoundingFrameV1::parse`
 * slices it by fixed stage widths and refuses on any other length, so a meta in
 * the wrong slot is a different instruction.
 *
 * ONE AUTHORITY. The stage widths, indices, magic, and the 129-account fixed
 * total all come from `lib/generated/genericFoundingV1.ts`, which
 * `scripts/generate-generic-founding.mjs` reads out of the on-chain route, the
 * codec crate, and the reference client. The *order* within each stage is the
 * one thing no constant carries; it is transcribed from
 * `tools/local-validator/bootstrap/successor/src/market.rs`'s
 * `build_generic_market_founding_v2`, the sole assembler of this frame in the
 * tree, and the two invariants that client asserts on itself — the exact width
 * and exactly eleven distinct writable keys — are asserted here too, so a
 * transcription slip that changes a privilege is caught rather than submitted.
 */

import { PublicKey, TransactionInstruction } from '@solana/web3.js';

import {
  CLAIMS_FOUNDING_ACCOUNT_COUNT_V5,
  GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1,
  GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1,
  GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1,
  GENERIC_MARKET_FOUNDING_DISTINCT_WRITABLE_V2,
  GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V2,
  GENERIC_MARKET_FOUNDING_INSTRUCTIONS_SYSVAR_INDEX_V2,
  GENERIC_MARKET_FOUNDING_MAGIC_V2,
  GENERIC_MARKET_FOUNDING_PREFIX_ACCOUNT_COUNT_V2,
  GENERIC_MARKET_FOUNDING_RAW_ACCOUNT_COUNT_V2,
  PROJECTED_FOUND_ACCOUNT_COUNT_V2,
  PROJECTED_CUSTODY_LOCK_CLOSE_ACCOUNT_COUNT_V1,
  PROJECTED_CUSTODY_REALIZE_ACCOUNT_COUNT_V1,
} from '../generated/genericFoundingV1';

export const INSTRUCTIONS_SYSVAR_ID = 'Sysvar1nstructions1111111111111111111111111';
export const CLOCK_SYSVAR_ID = 'SysvarC1ock11111111111111111111111111111111';
export const RENT_SYSVAR_ID = 'SysvarRent111111111111111111111111111111111';
export const SYSTEM_PROGRAM_ID = '11111111111111111111111111111111';

/** Every address the outer frame names, by the role the route reads it as. */
export type GenericMarketFoundingCoordinatesV2 = Readonly<{
  /** The four readonly content-addressed request records, in frame order. */
  foundRequest: string;
  lockRequest: string;
  realizeRequest: string;
  claimsRequest: string;

  /** Program roles and their Loader ProgramData accounts. */
  tradingProgram: string;
  tradingProgramData: string;
  coreProgram: string;
  coreProgramData: string;
  claimsProgram: string;
  claimsProgramData: string;
  custodyProgram: string;
  custodyProgramData: string;
  registryProgram: string;
  rentProgram: string;
  tokenProgram: string;
  activationCache: string;

  /** Stage caller PDAs, each derived from the request digest it authorizes. */
  lockCaller: string;
  realizeCaller: string;
  claimsCaller: string;
  foundAuthority: string;
  openAuthority: string;

  /** Market and Custody coordinates. */
  market: string;
  credit: string;
  permit: string;
  projectedReplay: string;
  sourceReplay: string;
  hoardVault: string;
  sourceVault: string;
  custodyAuthority: string;
  collateralMint: string;
  founder: string;

  /** Claims coordinates the founding allocates. */
  aggregate: string;
  position: string;
  admission: string;

  /** Registry raw/staging pairs the Claims stage authenticates. */
  liabilityBasisRaw: string;
  liabilityBasisStaging: string;
  productRaw: string;
  productStaging: string;
  resultDomainRaw: string;
  resultDomainStaging: string;
  portfolioRaw: string;
  portfolioStaging: string;

  /**
   * The 25-account compact ProjectedFound V2 frame, in its exact ABI order.
   *
   * Index 0 is the found authority rather than a payer and index 1 is the
   * Market the stage creates; both are writable, everything else is not. The
   * projection is the same one `prepareCoreFoundV2` builds, which is why this
   * builder takes it whole rather than re-deriving it: two derivations of one
   * frame is one derivation too many.
   */
  projectedFoundKeys: ReadonlyArray<string>;

  /** One canonical controller-subset FundingLedgerV2 per physical controller. */
  fundingLedgers: ReadonlyArray<string>;
}>;

/** The five CPI stages, plus the readonly prefix the route parses first. */
export type GenericMarketFoundingStageV2 = 'prefix' | 'lock' | 'found' | 'realize' | 'claims' | 'open';

export type GenericMarketFoundingAccountV2 = Readonly<{
  address: string;
  signer: boolean;
  writable: boolean;
  stage: GenericMarketFoundingStageV2;
  role: string;
}>;

export type GenericMarketFoundingStageBoundV2 = Readonly<{ start: number; count: number }>;

export type GenericMarketFoundingFrameV2 = Readonly<{
  programId: string;
  data: Uint8Array;
  accounts: ReadonlyArray<GenericMarketFoundingAccountV2>;
  fundingCount: number;
  distinctAccounts: ReadonlyArray<string>;
  distinctWritable: ReadonlyArray<string>;
  stageBounds: Readonly<Record<GenericMarketFoundingStageV2, GenericMarketFoundingStageBoundV2>>;
}>;

function canonical(value: string, field: string): string {
  const parsed = new PublicKey(value);
  if (parsed.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  return value;
}

type Emit = (address: string, writable: boolean, role: string) => void;

/**
 * Assemble the frame in the literal order `build_generic_market_founding_v1`
 * pushes it.
 *
 * Each stage is emitted through its own closure so the segment boundaries are
 * structural rather than counted, and the widths are then checked against the
 * generated constants: a stage that emits the wrong number of metas fails at
 * its own boundary and names itself, instead of shifting every later stage by
 * one and failing as a total-width mismatch.
 */
export function buildGenericMarketFoundingFrameV2(input: GenericMarketFoundingCoordinatesV2): GenericMarketFoundingFrameV2 {
  const fundingCount = input.fundingLedgers.length;
  if (fundingCount < 1 || fundingCount > 2) {
    throw new Error('the founding frame requires one or two controller-subset FundingLedgerV2 accounts');
  }
  if (input.projectedFoundKeys.length !== PROJECTED_FOUND_ACCOUNT_COUNT_V2) {
    throw new Error(`the Found stage requires exactly ${PROJECTED_FOUND_ACCOUNT_COUNT_V2} compact ProjectedFound V2 keys`);
  }

  const accounts: GenericMarketFoundingAccountV2[] = [];
  const bounds: Record<string, { start: number; count: number }> = {};
  let stage: GenericMarketFoundingStageV2 = 'prefix';
  const push: Emit = (address, writable, role) => {
    accounts.push(Object.freeze({ address: canonical(address, role), signer: false, writable, stage, role }));
  };
  function segment(name: GenericMarketFoundingStageV2, expected: number, body: (push: Emit) => void): void {
    const start = accounts.length;
    stage = name;
    body(push);
    const count = accounts.length - start;
    if (count !== expected) throw new Error(`the ${name} stage assembled ${count} accounts, not the ${expected} its ABI declares`);
    bounds[name] = { start, count };
  }

  segment('prefix', GENERIC_MARKET_FOUNDING_PREFIX_ACCOUNT_COUNT_V2, (emit) => {
    // Four readonly content-addressed requests, then the instructions sysvar
    // the heap-frame admission reads this transaction's own grant back out of.
    // The sysvar is not a convenience: `admit_heap_frame_v1` scans this
    // instruction's account list for it, and a founding that omits it keeps the
    // 32 KiB default ceiling and exhausts it three stages later.
    emit(input.foundRequest, false, 'Found request record');
    emit(input.lockRequest, false, 'terminal Lock request record');
    emit(input.realizeRequest, false, 'Realize request record');
    emit(input.claimsRequest, false, 'Claims FoundingV5 request record');
    emit(INSTRUCTIONS_SYSVAR_ID, false, 'instructions sysvar');
  });
  if (bounds.prefix.count !== GENERIC_MARKET_FOUNDING_RAW_ACCOUNT_COUNT_V2 + 1
      || accounts[GENERIC_MARKET_FOUNDING_INSTRUCTIONS_SYSVAR_INDEX_V2].address !== INSTRUCTIONS_SYSVAR_ID) {
    throw new Error('the instructions sysvar is not at the index the heap-frame admission scans');
  }

  segment('lock', PROJECTED_CUSTODY_LOCK_CLOSE_ACCOUNT_COUNT_V1, (emit) => {
    emit(input.lockCaller, false, 'Lock caller authority');
    emit(input.projectedReplay, true, 'projected Custody replay');
    emit(input.activationCache, false, 'release activation cache');
    emit(input.registryProgram, false, 'Registry program');
    emit(input.tradingProgram, false, 'Trading program');
    emit(input.tradingProgramData, false, 'Trading ProgramData');
    emit(input.credit, true, 'lifecycle RentCredit');
    emit(input.hoardVault, true, 'Hoard vault');
    emit(input.sourceVault, true, 'founding source vault');
    emit(input.custodyAuthority, false, 'Custody authority');
    emit(input.collateralMint, false, 'collateral Mint');
    emit(input.tokenProgram, false, 'Token program');
    emit(input.sourceReplay, true, 'founding source replay');
    emit(input.market, false, 'Market');
  });

  const foundCount = GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1 + fundingCount + GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1;
  segment('found', foundCount, (emit) => {
    input.projectedFoundKeys.forEach((key, index) => {
      // Index 0 is the found authority and index 1 is the Market the stage
      // creates; those two are the writable pair the compact frame declares.
      emit(key, index < 2, `ProjectedFound V2 account ${index.toString().padStart(2, '0')}`);
    });
    emit(input.tradingProgram, false, 'Trading program');
    emit(input.tradingProgramData, false, 'Trading ProgramData');
    emit(CLOCK_SYSVAR_ID, false, 'Clock sysvar');
    input.fundingLedgers.forEach((funding, index) => emit(funding, false, `FundingLedgerV2 ${index}`));
    emit(input.permit, true, 'one-shot Core permit');
    emit(input.projectedReplay, true, 'projected Custody replay');
    emit(input.hoardVault, true, 'Hoard vault');
    emit(input.sourceVault, true, 'founding source vault');
    emit(input.sourceReplay, true, 'founding source replay');
    emit(input.liabilityBasisRaw, false, 'linked liability basis raw');
    emit(input.liabilityBasisStaging, false, 'linked liability basis staging');
    emit(input.claimsProgram, false, 'Claims program');
    emit(input.claimsProgramData, false, 'Claims ProgramData');
    emit(input.custodyProgram, false, 'Custody program');
    emit(input.custodyProgramData, false, 'Custody ProgramData');
    emit(input.aggregate, true, 'Claims liability aggregate');
    emit(input.position, true, 'founder Position');
    emit(input.admission, true, 'Position admission');
    emit(input.founder, false, 'founder');
  });

  segment('realize', PROJECTED_CUSTODY_REALIZE_ACCOUNT_COUNT_V1, (emit) => {
    emit(input.realizeCaller, false, 'Realize caller authority');
    emit(input.projectedReplay, true, 'projected Custody replay');
    emit(input.activationCache, false, 'release activation cache');
    emit(input.registryProgram, false, 'Registry program');
    emit(input.tradingProgram, false, 'Trading program');
    emit(input.tradingProgramData, false, 'Trading ProgramData');
    emit(input.credit, true, 'lifecycle RentCredit');
    emit(input.hoardVault, true, 'Hoard vault');
    emit(input.market, false, 'Market');
    emit(input.custodyAuthority, false, 'Custody authority');
    emit(input.collateralMint, false, 'collateral Mint');
    emit(input.tokenProgram, false, 'Token program');
  });

  segment('claims', CLAIMS_FOUNDING_ACCOUNT_COUNT_V5, (emit) => {
    emit(input.claimsCaller, false, 'Claims caller authority');
    emit(input.permit, true, 'one-shot Core permit');
    emit(input.aggregate, true, 'Claims liability aggregate');
    emit(input.position, true, 'founder Position');
    emit(input.admission, true, 'Position admission');
    emit(input.sourceVault, true, 'founding source vault');
    emit(input.hoardVault, true, 'Hoard vault');
    emit(input.projectedReplay, true, 'projected Custody replay');
    emit(input.liabilityBasisRaw, false, 'linked liability basis raw');
    emit(input.liabilityBasisStaging, false, 'linked liability basis staging');
    emit(input.productRaw, false, 'Product raw');
    emit(input.productStaging, false, 'Product staging');
    emit(input.resultDomainRaw, false, 'result domain raw');
    emit(input.resultDomainStaging, false, 'result domain staging');
    emit(input.portfolioRaw, false, 'portfolio raw');
    emit(input.portfolioStaging, false, 'portfolio staging');
    emit(RENT_SYSVAR_ID, false, 'Rent sysvar');
    emit(SYSTEM_PROGRAM_ID, false, 'System program');
    emit(input.market, false, 'Market');
    emit(input.activationCache, false, 'release activation cache');
    emit(input.registryProgram, false, 'Registry program');
    emit(input.claimsProgram, false, 'Claims program');
    emit(input.claimsProgramData, false, 'Claims ProgramData');
    emit(input.coreProgram, false, 'Core program');
    emit(input.coreProgramData, false, 'Core ProgramData');
    emit(input.tradingProgram, false, 'Trading program');
    emit(input.tradingProgramData, false, 'Trading ProgramData');
    emit(input.custodyProgram, false, 'Custody program');
    emit(input.custodyProgramData, false, 'Custody ProgramData');
    emit(input.founder, false, 'founder');
    emit(input.credit, true, 'lifecycle RentCredit');
    emit(input.rentProgram, false, 'Rent program');
  });

  segment('open', GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1, (emit) => {
    emit(input.openAuthority, false, 'Open caller authority');
    emit(input.market, true, 'Market');
    emit(input.permit, true, 'one-shot Core permit');
    emit(input.credit, true, 'lifecycle RentCredit');
    emit(input.rentProgram, false, 'Rent program');
    emit(input.activationCache, false, 'release activation cache');
    emit(input.registryProgram, false, 'Registry program');
    emit(input.tradingProgram, false, 'Trading program');
    emit(input.tradingProgramData, false, 'Trading ProgramData');
    emit(input.claimsProgram, false, 'Claims program');
    emit(input.claimsProgramData, false, 'Claims ProgramData');
    emit(input.custodyProgram, false, 'Custody program');
    emit(input.custodyProgramData, false, 'Custody ProgramData');
    emit(input.coreProgram, false, 'Core program');
    emit(input.coreProgramData, false, 'Core ProgramData');
    emit(input.projectedReplay, true, 'projected Custody replay');
    emit(input.hoardVault, true, 'Hoard vault');
    emit(input.sourceVault, true, 'founding source vault');
    emit(input.aggregate, true, 'Claims liability aggregate');
    emit(input.position, true, 'founder Position');
    emit(input.admission, true, 'Position admission');
    emit(CLOCK_SYSVAR_ID, false, 'Clock sysvar');
    emit(RENT_SYSVAR_ID, false, 'Rent sysvar');
  });

  const expected = GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V2 + fundingCount;
  if (accounts.length !== expected) {
    throw new Error(`the assembled founding frame is ${accounts.length} accounts, not the ${expected} its ABI declares`);
  }

  // One key, one privilege: Solana grants writability per key for the whole
  // message, so a key writable in any stage is writable everywhere the frame
  // names it. Leaving a stage's copy readonly would not make it readonly; it
  // would only make this builder's account list disagree with the message the
  // runtime actually compiles.
  const writable = new Set(accounts.filter((account) => account.writable).map((account) => account.address));
  const unioned = accounts.map((account) => (writable.has(account.address) && !account.writable
    ? Object.freeze({ ...account, writable: true })
    : account));

  const distinctAccounts = [...new Set(unioned.map((account) => account.address))];
  const distinctWritable = [...writable];
  if (distinctWritable.length !== GENERIC_MARKET_FOUNDING_DISTINCT_WRITABLE_V2) {
    throw new Error(`the founding frame declared ${distinctWritable.length} writable keys, not the ${GENERIC_MARKET_FOUNDING_DISTINCT_WRITABLE_V2} the outer requires`);
  }
  // Admission belongs to the fully compiled v0 message. The frame cannot see
  // the fee payer, ComputeBudget program, or which keys the canonical ALT
  // loads, so it deliberately does not substitute a `distinct + 2` estimate.

  return Object.freeze({
    programId: canonical(input.tradingProgram, 'Trading program'),
    data: new TextEncoder().encode(GENERIC_MARKET_FOUNDING_MAGIC_V2),
    accounts: Object.freeze(unioned),
    fundingCount,
    distinctAccounts: Object.freeze(distinctAccounts),
    distinctWritable: Object.freeze(distinctWritable),
    stageBounds: Object.freeze(bounds) as GenericMarketFoundingFrameV2['stageBounds'],
  });
}

/** Lower one assembled frame to the instruction a versioned message carries. */
export function genericMarketFoundingInstructionV2(frame: GenericMarketFoundingFrameV2): TransactionInstruction {
  return new TransactionInstruction({
    programId: new PublicKey(frame.programId),
    keys: frame.accounts.map((account) => ({
      pubkey: new PublicKey(account.address),
      isSigner: account.signer,
      isWritable: account.writable,
    })),
    data: frame.data as Buffer,
  });
}
