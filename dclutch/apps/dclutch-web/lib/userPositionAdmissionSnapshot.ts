import { PublicKey } from '@solana/web3.js';

import { fromHex, slice } from './bytes';
import { acquireFinalizedAccountsInChunksV1 } from './coreFound';
import { decodeBase58 } from './explorer/base58';
import {
  GRADED_BASIS_RECORD_SCHEMA_ID_V3,
  LIABILITY_BASIS_MARKET_BASIS_OFFSET,
  LIABILITY_BASIS_MARKET_SEED_V2,
  LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
  PORTFOLIO_SCHEMA_ID_V2,
  PRODUCT_RECORD_DOMAIN_DIGEST_OFFSET_V2,
  PRODUCT_RECORD_PORTFOLIO_DIGEST_OFFSET_V2,
  PRODUCT_RECORD_SCHEMA_ID_V2,
  RESULT_DOMAIN_SCHEMA_ID_V2,
} from './generated/coreFound';
import {
  PROTOCOL_POSITION_ADMISSION_SEED_V2,
  PROTOCOL_POSITION_STATE_SEED_V2,
} from '@dclutch/sdk/generated/directParticipantV1';
import { USER_POSITION_ADMISSION_SNAPSHOT_FORMAT_V1 } from './generated/userPositionAdmissionWasmV1';
import { decodeMarketCoreStateV2 } from './marketCoreV2';
import {
  deriveFinalizedRecordAddressesV1,
  RENT_SYSVAR_ID,
  SYSTEM_PROGRAM_ID,
  UPGRADEABLE_LOADER_ID,
} from './releaseRegistry';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

/**
 * The twenty-five-account snapshot the compiled admission planner authenticates.
 *
 * THIS IS THE LAST UNIT BETWEEN A WALLET AND A TRADE. `JoinPanel` published a
 * CLI command because the browser could not assemble this frame; the planner
 * is compiled and digest-pinned now, and what it still needed was its input.
 *
 * TWO RULES, and both are the reason this file is longer than a fetch.
 *
 * EVERY ADDRESS IS DERIVED. The caller supplies a Market, an owner, and this
 * deployment's programs — eight strings, all of which the app already holds —
 * and every other coordinate is computed: the Claims aggregate from the
 * Market, the Position and admission records from the aggregate and the owner,
 * the four record raw/staging pairs from content digests under the Registry's
 * own PDA, the three ProgramData accounts from the Loader, and the RentCredit
 * from the Market and its generation. A snapshot a stranger pastes is a
 * snapshot a stranger can be silently wrong about, which is the rule that took
 * `/found` from fourteen pasted addresses to ten.
 *
 * EVERY READ IS FINALIZED, AT ONE FLOOR. The floor is taken once and passed to
 * every subsequent read as a minimum context slot. A snapshot stitched from
 * several observations authenticates a chain that existed at no single moment,
 * and one assembled from confirmed-but-not-finalized state authenticates a
 * chain that may not be the one that lands. The planner then refuses anything
 * whose observations disagree — this file's job is to not hand it that problem.
 *
 * Nothing here decides anything. The planner owns every check; this owns the
 * reads and the derivations, and the web shell keeps RPC, the wallet, durable
 * storage and submission.
 */

/** The snapshot's account fields, in the planner's own order. */
export const ADMISSION_SNAPSHOT_ACCOUNT_FIELDS_V1 = Object.freeze([
  'claimsMarket', 'position', 'admission',
  'linkedBasisRaw', 'linkedBasisStaging',
  'productRaw', 'productStaging',
  'resultDomainRaw', 'resultDomainStaging',
  'portfolioRaw', 'portfolioStaging',
  'rentSysvar', 'systemProgram', 'coreMarket', 'activationCache', 'registryProgram',
  'tradingProgram', 'tradingProgramdata',
  'claimsProgram', 'claimsProgramdata',
  'coreProgram', 'coreProgramdata',
  'owner', 'rentCredit', 'rentProgram',
] as const);

export type AdmissionSnapshotFieldV1 = (typeof ADMISSION_SNAPSHOT_ACCOUNT_FIELDS_V1)[number];

/** What a caller must already hold: a Market, a wallet, and this deployment. */
export type UserPositionAdmissionRequestV1 = Readonly<{
  market: string;
  owner: string;
  coreProgramId: string;
  claimsProgramId: string;
  tradingProgramId: string;
  registryProgramId: string;
  rentProgramId: string;
  activationCache: string;
}>;

/** The coordinates this module computed, for a reader to see before signing. */
export type UserPositionAdmissionDerivedV1 = Readonly<{
  claimsAggregate: string;
  position: string;
  admission: string;
  rentCredit: string;
  generation: string;
}>;

export type AcquiredAdmissionSnapshotV1 = Readonly<{
  snapshotJson: string;
  observedSlot: string;
  derived: UserPositionAdmissionDerivedV1;
}>;

function key(value: string, field: string): PublicKey {
  try { return new PublicKey(value); } catch { throw new Error(`${field} is not a base58 public key`); }
}

function pda(seeds: ReadonlyArray<Uint8Array>, programId: string, field: string): string {
  return PublicKey.findProgramAddressSync([...seeds], key(programId, field))[0].toBase58();
}

function loaderProgramData(programId: string): string {
  return PublicKey.findProgramAddressSync(
    [key(programId, 'role Program').toBytes()],
    key(UPGRADEABLE_LOADER_ID, 'Upgradeable Loader'),
  )[0].toBase58();
}

function required(accounts: ReadonlyMap<string, RpcAccount | null>, address: string, field: string): RpcAccount {
  const account = accounts.get(address);
  if (account === undefined || account === null) throw new Error(`${field} ${address} is absent at finalized commitment`);
  return account;
}

function base64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

function accountMap(observation: Awaited<ReturnType<SolanaRpcClient['multipleAccounts']>>): ReadonlyMap<string, RpcAccount | null> {
  return new Map(observation.accounts.map((entry) => [entry.address, entry.account]));
}

/** Assemble one finalized admission snapshot, deriving every coordinate. */
export async function acquireUserPositionAdmissionSnapshotV1(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'probe' | 'blockTime' | 'multipleAccounts'>,
  request: UserPositionAdmissionRequestV1,
): Promise<AcquiredAdmissionSnapshotV1> {
  const market = key(request.market, 'Market').toBase58();
  const owner = key(request.owner, 'owner wallet').toBase58();

  // ONE floor, taken before anything is read, and passed to every read below.
  const floor = await client.finalizedSlot();
  const facts = await client.probe();
  const unixTimestamp = (await client.blockTime(floor)) ?? '0';

  // 1 — the Market itself. Everything else hangs off its state.
  const coreObservation = await acquireFinalizedAccountsInChunksV1(client, [market], floor);
  const coreAccount = required(accountMap(coreObservation), market, 'Core Market');
  // Decoded by the app's own Core reader rather than by a second header check
  // written here. It owns the magic, the version, the accepted widths and the
  // superseded-generation vocabulary; a private copy of any of that would be
  // the browser becoming a second authority for the Market's own layout.
  let state;
  try { state = decodeMarketCoreStateV2(market, coreAccount.data); }
  catch (error) { throw new Error(`Core Market state has the wrong exact ABI: ${error instanceof Error ? error.message : 'unreadable'}`); }
  const identity = state.identity;
  // The Market names its own Registry program. Checking the deployment's
  // against it is a real check with two independent sources, and it refuses a
  // caller pointing this frame at a Registry the Market never selected.
  if (identity.registryProgram !== key(request.registryProgramId, 'Registry program').toBase58()) {
    throw new Error('the Market selects another Registry program than this deployment names');
  }
  const productRecordDigest = fromHex(identity.productRecordId, 'Product record digest');
  const generation = BigInt(identity.generation);

  // 2 — the coordinates the Market's own state determines.
  const claimsAggregate = pda([LIABILITY_BASIS_MARKET_SEED_V2, key(market, 'Market').toBytes()], request.claimsProgramId, 'Claims program');
  const product = deriveFinalizedRecordAddressesV1(request.registryProgramId, PRODUCT_RECORD_SCHEMA_ID_V2, productRecordDigest);

  // 3 — read the two records that name the rest, at the same floor.
  const graphObservation = await acquireFinalizedAccountsInChunksV1(client, [product.record, claimsAggregate], floor);
  const graph = accountMap(graphObservation);
  const productBytes = required(graph, product.record, 'Product record').data;
  const aggregateBytes = required(graph, claimsAggregate, 'Claims aggregate').data;
  const resultDomain = deriveFinalizedRecordAddressesV1(
    request.registryProgramId, RESULT_DOMAIN_SCHEMA_ID_V2, slice(productBytes, PRODUCT_RECORD_DOMAIN_DIGEST_OFFSET_V2, 32));
  const portfolio = deriveFinalizedRecordAddressesV1(
    request.registryProgramId, PORTFOLIO_SCHEMA_ID_V2, slice(productBytes, PRODUCT_RECORD_PORTFOLIO_DIGEST_OFFSET_V2, 32));
  const linkedBasis = deriveFinalizedRecordAddressesV1(
    request.registryProgramId, GRADED_BASIS_RECORD_SCHEMA_ID_V3, slice(aggregateBytes, LIABILITY_BASIS_MARKET_BASIS_OFFSET, 32));

  const generationBytes = new Uint8Array(8);
  new DataView(generationBytes.buffer).setBigUint64(0, generation, true);

  const derived: UserPositionAdmissionDerivedV1 = Object.freeze({
    claimsAggregate,
    position: pda([PROTOCOL_POSITION_STATE_SEED_V2, key(claimsAggregate, 'Claims aggregate').toBytes(), key(owner, 'owner').toBytes()], request.claimsProgramId, 'Claims program'),
    admission: pda([PROTOCOL_POSITION_ADMISSION_SEED_V2, key(claimsAggregate, 'Claims aggregate').toBytes(), key(owner, 'owner').toBytes()], request.claimsProgramId, 'Claims program'),
    rentCredit: pda([LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, key(market, 'Market').toBytes(), generationBytes], request.rentProgramId, 'Rent program'),
    generation: generation.toString(),
  });

  // 4 — the whole frame, in the planner's own field order.
  const coordinates: Readonly<Record<AdmissionSnapshotFieldV1, string>> = Object.freeze({
    claimsMarket: claimsAggregate,
    position: derived.position,
    admission: derived.admission,
    linkedBasisRaw: linkedBasis.record,
    linkedBasisStaging: linkedBasis.staging,
    productRaw: product.record,
    productStaging: product.staging,
    resultDomainRaw: resultDomain.record,
    resultDomainStaging: resultDomain.staging,
    portfolioRaw: portfolio.record,
    portfolioStaging: portfolio.staging,
    rentSysvar: RENT_SYSVAR_ID,
    systemProgram: SYSTEM_PROGRAM_ID,
    coreMarket: market,
    activationCache: key(request.activationCache, 'activation cache').toBase58(),
    registryProgram: key(request.registryProgramId, 'Registry program').toBase58(),
    tradingProgram: key(request.tradingProgramId, 'Trading program').toBase58(),
    tradingProgramdata: loaderProgramData(request.tradingProgramId),
    claimsProgram: key(request.claimsProgramId, 'Claims program').toBase58(),
    claimsProgramdata: loaderProgramData(request.claimsProgramId),
    coreProgram: key(request.coreProgramId, 'Core program').toBase58(),
    coreProgramdata: loaderProgramData(request.coreProgramId),
    owner,
    rentCredit: derived.rentCredit,
    rentProgram: key(request.rentProgramId, 'Rent program').toBase58(),
  });

  const addresses = ADMISSION_SNAPSHOT_ACCOUNT_FIELDS_V1.map((field) => coordinates[field]);
  if (new Set(addresses).size !== addresses.length) {
    // Two coordinates that collapse to one address is a derivation that went
    // wrong, and the planner would authenticate the alias rather than the pair.
    throw new Error('admission frame derives two coordinates to the same address');
  }
  const frame = accountMap(await acquireFinalizedAccountsInChunksV1(client, addresses, floor));

  const wire: Record<string, unknown> = {
    format: USER_POSITION_ADMISSION_SNAPSHOT_FORMAT_V1,
    genesisHash: base64(decodeBase58(facts.genesisHash)),
  };
  for (const field of ADMISSION_SNAPSHOT_ACCOUNT_FIELDS_V1) {
    const address = coordinates[field];
    // A vacant PDA is a legitimate input — the Position and the admission
    // record MUST be vacant — so absence is carried, not refused. The planner
    // decides which of the twenty-five may be empty; this file does not.
    const account = frame.get(address) ?? null;
    wire[field] = {
      observation: { slot: floor, unixTimestamp, finality: 'finalized' },
      key: address,
      owner: account?.owner ?? SYSTEM_PROGRAM_ID,
      lamports: account?.lamports ?? '0',
      executable: account?.executable ?? false,
      dataBase64: base64(account?.data ?? new Uint8Array(0)),
    };
  }

  return Object.freeze({
    snapshotJson: JSON.stringify(wire),
    observedSlot: floor,
    derived,
  });
}

