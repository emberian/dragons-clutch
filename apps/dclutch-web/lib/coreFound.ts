import {
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';

import { PACKET_DATA_SIZE } from './directTransaction';
import {
  CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
  ARTIFACT_RELEASE_SCHEMA_ID_V1,
  CREATE_LIFECYCLE_RENT_CREDIT_BYTES_V2,
  CORE_ACTION_FOUND_TAG,
  CORE_FOUND_ACCOUNT_COUNT_V2,
  CORE_REQUEST_BYTES,
  CORE_REQUEST_MAGIC,
  CORE_STATE_BYTES,
  CORE_VERSION,
  EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1,
  LIFECYCLE_RENT_CREDIT_BYTES_V2,
  LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
  LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2,
  LIFECYCLE_RENT_SCHEMA_VERSION_V2,
  MARKET_CORE_STATE_PDA_DOMAIN_V2,
  PORTFOLIO_SCHEMA_ID_V2,
  PRODUCT_RECORD_SCHEMA_ID_V2,
  REALM_SCHEMA_RELEASE_ID_V1,
  RESULT_DOMAIN_SCHEMA_ID_V2,
  SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
} from './generated/coreFound';
import { inspectProtocolInfrastructureV1, type ProtocolInfrastructureInspectionV1 } from './infrastructure';
import {
  NATIVE_LOADER_ID,
  RENT_SYSVAR_ID,
  SYSTEM_PROGRAM_ID,
  SYSVAR_OWNER_ID,
  decodeExecutionReleaseSetV1,
  deriveFinalizedRecordAddressesV1,
} from './releaseRegistry';
import { type RpcAccount, type SolanaRpcClient } from './rpc';
import { ascii, hex, isZero, requireNonzero, requireZero, sha256, slice, u16, u64 } from './bytes';

const PRODUCT_RECORD_BYTES = 112;
const DOMAIN_HEADER_BYTES = 240;
const PORTFOLIO_HEADER_BYTES = 208;
const SOURCE_MATERIAL_BYTES = 208;
const REALM_BYTES = 112;
const MANIFEST_HEADER_BYTES = 16;
const MANIFEST_ENTRY_BYTES = 528;
const MAX_U32 = 0xffff_ffff;

export type CoreFoundInputV2 = Readonly<{
  payer: string;
  registryProgram: string;
  activationCache: string;
  refundWallet: string;
  realmRecord: string;
  productRecord: string;
  resultDomainRecord: string;
  portfolioRecord: string;
  sourceMaterialRecord: string;
  capabilityManifestRecord: string;
  executionReleaseSetRecord: string;
  generation: bigint;
}>;

export type CoreFoundPlanV2 = Readonly<{
  observedSlot: string;
  market: string;
  rentCredit: string;
  coreProgram: string;
  registryProgram: string;
  rentProgram: string;
  productRecordDigest: string;
  productId: string;
  outcomeCount: number;
  executionReleaseSetId: string;
  infrastructureProfile: string;
  infrastructureRecognition: ProtocolInfrastructureInspectionV1['recognition'];
  marketRentTopUp: string;
  rentCreditRentDebit: string;
  lastValidBlockHeight: string;
  accountAddresses: ReadonlyArray<string>;
  requiredSigners: ReadonlyArray<string>;
  requestBytes: Uint8Array;
  rentCreateRequestBytes: Uint8Array;
  rentCreateTransaction: VersionedTransaction;
  rentCreateWireBytes: Uint8Array;
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
}>;

export type CompiledCoreFoundTransactionV2 = Readonly<{
  requestBytes: Uint8Array;
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  requiredSigners: ReadonlyArray<string>;
}>;

export type CompiledLifecycleRentCreateTransactionV2 = Readonly<{
  rentCredit: string;
  requestBytes: Uint8Array;
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  requiredSigners: ReadonlyArray<string>;
}>;

type RecordAuthority = Readonly<{
  raw: string;
  staging: string;
  schema: Uint8Array;
  digest: Uint8Array;
  bytes: Uint8Array;
  rentMinimum: bigint;
}>;

type ProductGraph = Readonly<{ productId: Uint8Array; outcomeCount: number }>;

function key(value: string, field: string): PublicKey {
  const parsed = new PublicKey(value);
  if (parsed.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  return parsed;
}

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function accountMap(entries: Awaited<ReturnType<SolanaRpcClient['multipleAccounts']>>): ReadonlyMap<string, RpcAccount | null> {
  return new Map(entries.accounts.map((entry) => [entry.address, entry.account]));
}

function required(accounts: ReadonlyMap<string, RpcAccount | null>, address: string, field: string): RpcAccount {
  const account = accounts.get(address);
  if (account === undefined || account === null) throw new Error(`${field} ${address} is absent at finalized commitment`);
  return account;
}

function vacant(account: RpcAccount | null | undefined, field: string): void {
  if (account === null || account === undefined) return;
  if (account.owner !== SYSTEM_PROGRAM_ID || account.executable || account.data.length !== 0) {
    throw new Error(`${field} is not a vacant System-owned account`);
  }
}

function u32(bytes: Uint8Array, offset: number): number {
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0, true);
}

function i128(bytes: Uint8Array, offset: number): bigint {
  const view = new DataView(bytes.buffer, bytes.byteOffset + offset, 16);
  return (view.getBigInt64(8, true) << 64n) + view.getBigUint64(0, true);
}

function putU16(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 2).setUint16(0, value, true);
}

function putU64(bytes: Uint8Array, offset: number, value: bigint): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 8).setBigUint64(0, value, true);
}

function compareBytes(left: Uint8Array, right: Uint8Array): number {
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) return left[index] < right[index] ? -1 : 1;
  }
  return 0;
}

function gcd(left: bigint, right: bigint): bigint {
  let a = left;
  let b = right;
  while (b !== 0n) [a, b] = [b, a % b];
  return a;
}

async function recordAuthority(
  client: Pick<SolanaRpcClient, 'minimumBalanceForRentExemption'>,
  registry: string,
  raw: string,
  schema: Uint8Array,
  account: RpcAccount,
  field: string,
): Promise<RecordAuthority> {
  if (account.owner !== registry || account.executable) throw new Error(`${field} is not a nonexecutable Registry-owned raw record`);
  const digest = await sha256(account.data);
  const coordinates = deriveFinalizedRecordAddressesV1(registry, schema, digest);
  if (coordinates.record !== raw) throw new Error(`${field} is not the schema/content-derived Registry raw PDA`);
  const rent = await client.minimumBalanceForRentExemption(account.data.length);
  if (BigInt(account.lamports) < BigInt(rent.lamports)) throw new Error(`${field} is below its exact current rent minimum`);
  return Object.freeze({ raw, staging: coordinates.staging, schema, digest, bytes: account.data, rentMinimum: BigInt(rent.lamports) });
}

function validateRealm(bytes: Uint8Array): void {
  if (bytes.length !== REALM_BYTES || ascii(bytes, 0, 8) !== 'DCLTRLM1' || u16(bytes, 8) !== 1) throw new Error('Realm record has the wrong exact ABI');
  if (bytes[10] > 1 || bytes[11] > 1) throw new Error('Realm authority policy is undefined');
  requireZero(bytes, 12, 4, 'Realm header');
  [16, 48, 80].forEach((offset) => requireNonzero(slice(bytes, offset, 32), 'Realm identity'));
}

export function validateCoreFoundSourceMaterialV2(bytes: Uint8Array, productDigest: Uint8Array): void {
  if (bytes.length !== SOURCE_MATERIAL_BYTES || ascii(bytes, 0, 8) !== 'DCLTSMV2' || u16(bytes, 8) !== 2) throw new Error('SourceMaterialV2 has the wrong exact ABI');
  if (bytes[10] > 1) throw new Error('SourceMaterialV2 recovery tag is undefined');
  requireZero(bytes, 11, 5, 'SourceMaterialV2 header');
  if (!same(slice(bytes, 16, 32), productDigest)) throw new Error('SourceMaterialV2 selects a different Product record digest');
  [16, 48, 80, 112, 176].forEach((offset) => requireNonzero(slice(bytes, offset, 32), 'SourceMaterialV2 identity'));
  const recovery = slice(bytes, 144, 32);
  if ((bytes[10] === 0 && !isZero(recovery)) || (bytes[10] === 1 && isZero(recovery))) throw new Error('SourceMaterialV2 recovery policy is noncanonical');
}

function validateFundingQuote(bytes: Uint8Array): Readonly<{ rent: bigint; creation: bigint }> {
  if (bytes.length !== 304 || ascii(bytes, 0, 8) !== 'DCLTFQ01' || u16(bytes, 8) !== 1 || bytes[10] > 1) throw new Error('capability funding quote has the wrong exact ABI');
  requireZero(bytes, 11, 5, 'capability funding quote header');
  const binding = slice(bytes, 16, 160);
  if (bytes[10] === 0) requireZero(binding, 0, 160, 'absent Realm funding binding');
  else [0, 32, 64, 96, 128].forEach((offset) => requireNonzero(slice(binding, offset, 32), 'Realm funding binding'));
  let nativeTotal = 0n;
  let realmTotal = 0n;
  const amounts: bigint[] = [];
  for (let index = 0; index < 7; index += 1) {
    const offset = 176 + index * 16;
    const asset = bytes[offset];
    const amount = u64(bytes, offset + 8);
    requireZero(bytes, offset + 1, 7, 'capability funding allocation');
    if (asset > 2 || (amount === 0n) !== (asset === 0) || (index < 2 && asset === 2)) throw new Error('capability funding allocation has a noncanonical asset class');
    if (asset === 1) nativeTotal += amount;
    if (asset === 2) realmTotal += amount;
    if (nativeTotal > 0xffff_ffff_ffff_ffffn || realmTotal > 0xffff_ffff_ffff_ffffn) {
      throw new Error('capability funding compartment total overflows u64');
    }
    amounts.push(amount);
  }
  if (u64(bytes, 288) !== nativeTotal || u64(bytes, 296) !== realmTotal) throw new Error('capability funding totals do not equal their typed compartments');
  if ((realmTotal === 0n) !== (bytes[10] === 0)) throw new Error('Realm funding binding does not match Realm collateral use');
  return Object.freeze({ rent: amounts[0], creation: amounts[1] });
}

export function validateCoreFoundCapabilityManifestV1(bytes: Uint8Array): void {
  if (bytes.length < MANIFEST_HEADER_BYTES || ascii(bytes, 0, 8) !== 'DCLTCAP1' || u16(bytes, 8) !== 1 || u16(bytes, 10) !== 1) throw new Error('capability manifest has the wrong exact header');
  requireZero(bytes, 14, 2, 'capability manifest header');
  const count = u16(bytes, 12);
  if (count > 16 || bytes.length !== MANIFEST_HEADER_BYTES + count * MANIFEST_ENTRY_BYTES) throw new Error('capability manifest width is not canonical');
  const dependencies: number[][] = [];
  let previous: Uint8Array | null = null;
  for (let index = 0; index < count; index += 1) {
    const offset = MANIFEST_HEADER_BYTES + index * MANIFEST_ENTRY_BYTES;
    const kind = slice(bytes, offset, 32);
    [0, 32, 64, 96, 128, 160].forEach((relative) => requireNonzero(slice(bytes, offset + relative, 32), 'capability entry identity'));
    if (previous !== null && compareBytes(previous, kind) >= 0) throw new Error('capability entries are not strictly ordered');
    previous = kind;
    const policy = bytes[offset + 192];
    const dependencyCount = bytes[offset + 193];
    if (policy > 1 || dependencyCount > 16) throw new Error('capability entry policy or dependency count is undefined');
    requireZero(bytes, offset + 194, 6, 'capability entry header');
    const active: number[] = [];
    for (let position = 0; position < 16; position += 1) {
      const dependency = bytes[offset + 208 + position];
      if (position < dependencyCount) {
        if (dependency >= count || dependency === index || (active.length > 0 && active[active.length - 1] >= dependency)) throw new Error('capability dependency is invalid or noncanonical');
        active.push(dependency);
      } else if (dependency !== 0) throw new Error('inactive capability dependency is nonzero');
    }
    const deadline = u64(bytes, offset + 200);
    const quote = validateFundingQuote(slice(bytes, offset + 224, 304));
    if ((policy === 0 && deadline !== 0n) || (policy === 1 && (deadline === 0n || (quote.rent === 0n && quote.creation === 0n)))) throw new Error('capability activation policy and prepaid creation facts do not join');
    dependencies.push(active);
  }
  const resolved = new Set<number>();
  while (resolved.size < count) {
    const before = resolved.size;
    dependencies.forEach((entry, index) => { if (!resolved.has(index) && entry.every((dependency) => resolved.has(dependency))) resolved.add(index); });
    if (resolved.size === before) throw new Error('capability dependency graph is cyclic');
  }
}

export function decodeCoreFoundProductGraphV2(product: Uint8Array, domain: Uint8Array, portfolio: Uint8Array, domainDigest: Uint8Array, portfolioDigest: Uint8Array): ProductGraph {
  if (product.length !== PRODUCT_RECORD_BYTES || ascii(product, 0, 8) !== 'DCLTPRM2' || u16(product, 8) !== 2) throw new Error('Product Runtime V2 root has the wrong exact ABI');
  requireZero(product, 10, 6, 'Product Runtime V2 root');
  const productId = slice(product, 16, 32);
  requireNonzero(productId, 'Product identity');
  if (!same(slice(product, 48, 32), domainDigest) || !same(slice(product, 80, 32), portfolioDigest)) throw new Error('Product root does not select the supplied domain and portfolio');

  if (domain.length < DOMAIN_HEADER_BYTES || ascii(domain, 0, 8) !== 'DCLTPRD2' || u16(domain, 8) !== 2 || u16(domain, 10) !== DOMAIN_HEADER_BYTES || u32(domain, 12) !== domain.length) throw new Error('Product result domain has the wrong exact ABI');
  requireZero(domain, 24, 8, 'Product result-domain header');
  requireZero(domain, 232, 8, 'Product result-domain tail');
  const regions = u32(domain, 16);
  const cuts = u32(domain, 20);
  if (regions === 0 || regions !== cuts + 1 || domain.length !== DOMAIN_HEADER_BYTES + cuts * 16) throw new Error('Product result-domain width is inconsistent');
  [32, 64, 96, 128, 160, 192].forEach((offset) => requireNonzero(slice(domain, offset, 32), 'Product result-domain identity'));
  if (!same(slice(domain, 32, 32), productId) || u64(domain, 224) === 0n) throw new Error('Product result-domain identity or denominator differs');
  let prior: bigint | null = null;
  for (let index = 0; index < cuts; index += 1) {
    const cut = i128(domain, DOMAIN_HEADER_BYTES + index * 16);
    if (prior !== null && cut <= prior) throw new Error('Product result-domain cuts are not strictly increasing');
    prior = cut;
  }

  if (portfolio.length < PORTFOLIO_HEADER_BYTES || ascii(portfolio, 0, 8) !== 'DCLTPRF2' || u16(portfolio, 8) !== 2 || u16(portfolio, 10) !== PORTFOLIO_HEADER_BYTES || u32(portfolio, 12) !== portfolio.length) throw new Error('Product portfolio has the wrong exact ABI');
  const coefficientCount = u32(portfolio, 16);
  if (coefficientCount === 0 || coefficientCount !== regions + 1 || portfolio.length !== PORTFOLIO_HEADER_BYTES + coefficientCount * 8 || portfolio[20] !== 1) throw new Error('Product portfolio width or rounding boundary is inconsistent');
  requireZero(portfolio, 21, 11, 'Product portfolio header');
  requireZero(portfolio, 200, 8, 'Product portfolio tail');
  [32, 64, 96, 128, 160].forEach((offset) => requireNonzero(slice(portfolio, offset, 32), 'Product portfolio identity'));
  if (!same(slice(portfolio, 32, 32), productId) || !same(slice(portfolio, 64, 32), domainDigest) || !same(slice(portfolio, 128, 32), slice(domain, 128, 32)) || !same(slice(portfolio, 160, 32), slice(domain, 160, 32))) throw new Error('Product domain and portfolio identities do not join');
  let divisor = u64(portfolio, 192);
  let nonzero = false;
  if (divisor === 0n) throw new Error('Product portfolio denominator is zero');
  for (let index = 0; index < coefficientCount; index += 1) {
    const coefficient = u64(portfolio, PORTFOLIO_HEADER_BYTES + index * 8);
    nonzero ||= coefficient !== 0n;
    divisor = gcd(divisor, coefficient);
  }
  if (!nonzero || divisor !== 1n) throw new Error('Product portfolio is empty or not gcd-normalized');
  const outcomeCount = regions + 1;
  if (outcomeCount > MAX_U32) throw new Error('Product outcome width exceeds u32');
  return Object.freeze({ productId, outcomeCount });
}

function generationBytes(generation: bigint): Uint8Array {
  if (generation <= 0n || generation > 0xffff_ffff_ffff_ffffn) throw new Error('Market generation is outside lifecycle u64');
  const bytes = new Uint8Array(8);
  putU64(bytes, 0, generation);
  return bytes;
}

function lifecycleRentCreateRequest(input: Readonly<{
  refundWallet: PublicKey;
  market: PublicKey;
  releaseSet: Uint8Array;
  generation: bigint;
  bump: number;
}>): Uint8Array {
  if (input.releaseSet.length !== 32 || isZero(input.releaseSet)) throw new Error('execution release set is not one nonzero 32-byte identity');
  if (same(input.refundWallet.toBytes(), input.market.toBytes())
      || same(input.refundWallet.toBytes(), input.releaseSet)
      || same(input.market.toBytes(), input.releaseSet)) {
    throw new Error('lifecycle Rent identities alias');
  }
  const output = new Uint8Array(CREATE_LIFECYCLE_RENT_CREDIT_BYTES_V2);
  output.set(LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2, 0);
  putU16(output, 8, LIFECYCLE_RENT_SCHEMA_VERSION_V2);
  output[10] = 0;
  output.set(input.refundWallet.toBytes(), 16);
  output.set(input.market.toBytes(), 48);
  output.set(input.releaseSet, 80);
  output.set(generationBytes(input.generation), 112);
  output[120] = input.bump;
  return output;
}

export function compileLifecycleRentCreateTransactionV2(input: Readonly<{
  payer: string;
  refundWallet: string;
  market: string;
  releaseSet: Uint8Array;
  generation: bigint;
  rentProgram: string;
  recentBlockhash: string;
}>): CompiledLifecycleRentCreateTransactionV2 {
  const payer = key(input.payer, 'payer');
  const refundWallet = key(input.refundWallet, 'refund wallet');
  const market = key(input.market, 'Market');
  const rentProgram = key(input.rentProgram, 'Rent program');
  const generation = generationBytes(input.generation);
  const [rentCredit, bump] = PublicKey.findProgramAddressSync(
    [LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, market.toBytes(), generation],
    rentProgram,
  );
  const requestBytes = lifecycleRentCreateRequest({
    refundWallet,
    market,
    releaseSet: input.releaseSet,
    generation: input.generation,
    bump,
  });
  const instruction = new TransactionInstruction({
    programId: rentProgram,
    keys: [
      { pubkey: payer, isSigner: true, isWritable: true },
      { pubkey: rentCredit, isSigner: false, isWritable: true },
      { pubkey: key(SYSTEM_PROGRAM_ID, 'System program'), isSigner: false, isWritable: false },
      { pubkey: key(RENT_SYSVAR_ID, 'Rent sysvar'), isSigner: false, isWritable: false },
    ],
    data: requestBytes as Buffer,
  });
  const transaction = new VersionedTransaction(new TransactionMessage({
    payerKey: payer,
    recentBlockhash: key(input.recentBlockhash, 'recent blockhash').toBase58(),
    instructions: [instruction],
  }).compileToV0Message());
  const wireBytes = transaction.serialize();
  if (wireBytes.length > PACKET_DATA_SIZE) throw new Error('lifecycle Rent Create exceeds the packet bound');
  const requiredSigners = Object.freeze(transaction.message.staticAccountKeys
    .slice(0, transaction.message.header.numRequiredSignatures)
    .map((value) => value.toBase58()));
  if (requiredSigners.length !== 1 || requiredSigners[0] !== input.payer) throw new Error('lifecycle Rent Create requires an unexpected signer');
  return Object.freeze({ rentCredit: rentCredit.toBase58(), requestBytes, transaction, wireBytes, requiredSigners });
}

function foundRequest(generation: bigint, market: PublicKey): Uint8Array {
  if (generation < 0n || generation > 0xffff_ffff_ffff_ffffn) throw new Error('Market generation is outside u64');
  const output = new Uint8Array(CORE_REQUEST_BYTES);
  output.set(CORE_REQUEST_MAGIC, 0);
  putU16(output, 8, CORE_VERSION);
  output[10] = CORE_ACTION_FOUND_TAG;
  putU64(output, 32, generation);
  output.set(market.toBytes(), 40);
  return output;
}

export function compileCoreFoundTransactionV2(input: Readonly<{
  payer: string;
  coreProgram: string;
  market: string;
  generation: bigint;
  recentBlockhash: string;
  accountAddresses: ReadonlyArray<string>;
}>): CompiledCoreFoundTransactionV2 {
  if (input.accountAddresses.length !== CORE_FOUND_ACCOUNT_COUNT_V2) {
    throw new Error(`Core Found requires exactly ${CORE_FOUND_ACCOUNT_COUNT_V2} account metas`);
  }
  if (new Set(input.accountAddresses).size !== CORE_FOUND_ACCOUNT_COUNT_V2) {
    throw new Error('Core Found account metas alias named roles');
  }
  if (input.accountAddresses[0] !== input.payer
      || input.accountAddresses[1] !== input.market
      || input.accountAddresses[19] !== input.coreProgram) {
    throw new Error('Core Found payer, Market, or Core program is at the wrong exact account index');
  }
  const payer = key(input.payer, 'payer');
  const market = key(input.market, 'Market');
  const requestBytes = foundRequest(input.generation, market);
  const metas = input.accountAddresses.map((address, index) => ({
    pubkey: key(address, `Found account ${index}`),
    isSigner: index === 0,
    isWritable: index === 0 || index === 1,
  }));
  const instruction = new TransactionInstruction({
    programId: key(input.coreProgram, 'Core program'),
    keys: metas,
    data: requestBytes as Buffer,
  });
  const transaction = new VersionedTransaction(new TransactionMessage({
    payerKey: payer,
    recentBlockhash: key(input.recentBlockhash, 'recent blockhash').toBase58(),
    instructions: [instruction],
  }).compileToV0Message());
  const wireBytes = transaction.serialize();
  if (wireBytes.length > PACKET_DATA_SIZE) {
    throw new Error(`Core Found transaction is ${wireBytes.length} bytes, above the ${PACKET_DATA_SIZE}-byte packet bound`);
  }
  const requiredSigners = Object.freeze(transaction.message.staticAccountKeys
    .slice(0, transaction.message.header.numRequiredSignatures)
    .map((value) => value.toBase58()));
  if (requiredSigners.length !== 1 || requiredSigners[0] !== input.payer) {
    throw new Error('Core Found transaction requires an unexpected signer');
  }
  return Object.freeze({ requestBytes, transaction, wireBytes, requiredSigners });
}

function infrastructureEqual(left: ProtocolInfrastructureInspectionV1, right: ProtocolInfrastructureInspectionV1): boolean {
  return left.registryProgram === right.registryProgram
    && left.activationCache === right.activationCache
    && left.executionReleaseSetId === right.executionReleaseSetId
    && left.profilePda === right.profilePda
    && left.profileDigest === right.profileDigest
    && JSON.stringify(left.core) === JSON.stringify(right.core)
    && JSON.stringify(left.registry) === JSON.stringify(right.registry)
    && JSON.stringify(left.rent) === JSON.stringify(right.rent);
}

export async function prepareCoreFoundV2(client: SolanaRpcClient, input: CoreFoundInputV2): Promise<CoreFoundPlanV2> {
  generationBytes(input.generation);
  key(input.payer, 'payer');
  const registry = key(input.registryProgram, 'Registry program');
  key(input.activationCache, 'activation cache');
  key(input.refundWallet, 'refund wallet');
  const rawAddresses = [input.realmRecord, input.productRecord, input.resultDomainRecord, input.portfolioRecord, input.sourceMaterialRecord, input.capabilityManifestRecord, input.executionReleaseSetRecord];
  rawAddresses.forEach((address, index) => key(address, `finalized raw record ${index}`));
  if (new Set([input.registryProgram, input.activationCache, input.refundWallet, ...rawAddresses]).size !== 10) throw new Error('Found authority inputs alias named roles');

  const infrastructure = await inspectProtocolInfrastructureV1(client, { registryProgram: registry.toBase58(), activationCache: input.activationCache });
  const initial = await client.multipleAccounts(rawAddresses, infrastructure.observedSlot);
  const initialAccounts = accountMap(initial);
  const specs = [
    [input.realmRecord, REALM_SCHEMA_RELEASE_ID_V1, 'Realm'],
    [input.productRecord, PRODUCT_RECORD_SCHEMA_ID_V2, 'Product'],
    [input.resultDomainRecord, RESULT_DOMAIN_SCHEMA_ID_V2, 'result domain'],
    [input.portfolioRecord, PORTFOLIO_SCHEMA_ID_V2, 'portfolio'],
    [input.sourceMaterialRecord, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2, 'Source material'],
    [input.capabilityManifestRecord, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, 'capability manifest'],
  ] as const;
  const authorities = await Promise.all(specs.map(([address, schema, field]) => recordAuthority(client, input.registryProgram, address, schema, required(initialAccounts, address, field), field)));
  const releaseSetAccount = required(initialAccounts, input.executionReleaseSetRecord, 'execution release set');
  const releaseSetDecoded = await decodeExecutionReleaseSetV1(releaseSetAccount.data);
  const releaseSetAuthority = await recordAuthority(client, input.registryProgram, input.executionReleaseSetRecord, EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1, releaseSetAccount, 'execution release set');
  if (releaseSetDecoded.id !== hex(releaseSetAuthority.digest) || releaseSetDecoded.id !== infrastructure.executionReleaseSetId) throw new Error('finalized release set and activation cache identities differ');

  const [realm, product, domain, portfolio, source, manifest] = authorities;
  validateRealm(realm.bytes);
  const graph = decodeCoreFoundProductGraphV2(product.bytes, domain.bytes, portfolio.bytes, domain.digest, portfolio.digest);
  validateCoreFoundSourceMaterialV2(source.bytes, product.digest);
  validateCoreFoundCapabilityManifestV1(manifest.bytes);

  const market = PublicKey.findProgramAddressSync([
    MARKET_CORE_STATE_PDA_DOMAIN_V2,
    realm.digest,
    product.digest,
    graph.productId,
    source.digest,
    manifest.digest,
    releaseSetAuthority.digest,
    registry.toBytes(),
    generationBytes(input.generation),
  ], key(infrastructure.core.program, 'Core program'))[0];
  const [rentCredit] = PublicKey.findProgramAddressSync([
    LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
    market.toBytes(),
    generationBytes(input.generation),
  ], key(infrastructure.rent.program, 'Rent program'));
  const registryArtifact = deriveFinalizedRecordAddressesV1(input.registryProgram, ARTIFACT_RELEASE_SCHEMA_ID_V1, Uint8Array.from(infrastructure.registry.artifactReleaseId.match(/../g) ?? [], (value) => Number.parseInt(value, 16)));
  const rentArtifact = deriveFinalizedRecordAddressesV1(input.registryProgram, ARTIFACT_RELEASE_SCHEMA_ID_V1, Uint8Array.from(infrastructure.rent.artifactReleaseId.match(/../g) ?? [], (value) => Number.parseInt(value, 16)));
  const accountAddresses = [
    input.payer, market.toBase58(), rentCredit.toBase58(), infrastructure.rent.program,
    realm.raw, realm.staging, product.raw, product.staging, domain.raw, domain.staging, portfolio.raw, portfolio.staging,
    source.raw, source.staging, manifest.raw, manifest.staging, releaseSetAuthority.raw, releaseSetAuthority.staging,
    input.activationCache, infrastructure.core.program, infrastructure.core.programData, input.registryProgram, RENT_SYSVAR_ID, SYSTEM_PROGRAM_ID,
    infrastructure.profilePda, registryArtifact.record, registryArtifact.staging, infrastructure.registry.programData,
    rentArtifact.record, rentArtifact.staging, infrastructure.rent.programData,
  ];
  if (accountAddresses.length !== CORE_FOUND_ACCOUNT_COUNT_V2 || new Set(accountAddresses).size !== CORE_FOUND_ACCOUNT_COUNT_V2) throw new Error('Found31 account projection aliases named roles');
  const observationAddresses = input.refundWallet === input.payer ? accountAddresses : [...accountAddresses, input.refundWallet];
  const finalObservation = await client.multipleAccounts(observationAddresses, initial.slot);
  const finalAccounts = accountMap(finalObservation);
  const payerAccount = required(finalAccounts, input.payer, 'payer');
  if (payerAccount.owner !== SYSTEM_PROGRAM_ID || payerAccount.executable || payerAccount.data.length !== 0) throw new Error('payer is not a System-owned data-free wallet');
  vacant(finalAccounts.get(market.toBase58()), 'Market destination');
  const refundWallet = required(finalAccounts, input.refundWallet, 'refund wallet');
  if (refundWallet.owner !== SYSTEM_PROGRAM_ID || refundWallet.executable || refundWallet.data.length !== 0) throw new Error('refund wallet is not a System-owned data-free wallet');
  const creditDestination = finalAccounts.get(rentCredit.toBase58());
  vacant(creditDestination, 'lifecycle RentCredit destination');
  if ((creditDestination?.lamports ?? '0') !== '0') throw new Error('lifecycle RentCredit destination is prefunded');
  for (const authority of [...authorities, releaseSetAuthority]) {
    const raw = required(finalAccounts, authority.raw, 'finalized raw record');
    if (raw.owner !== input.registryProgram || raw.executable || BigInt(raw.lamports) < authority.rentMinimum || !same(raw.data, authority.bytes) || !same(await sha256(raw.data), authority.digest)) throw new Error('finalized raw record changed or lost Registry/rent authority');
    vacant(finalAccounts.get(authority.staging), 'finalized staging cursor');
  }
  const rentSysvar = required(finalAccounts, RENT_SYSVAR_ID, 'Rent sysvar');
  const system = required(finalAccounts, SYSTEM_PROGRAM_ID, 'System Program');
  if (rentSysvar.owner !== SYSVAR_OWNER_ID || rentSysvar.executable || rentSysvar.data.length !== 17 || system.owner !== NATIVE_LOADER_ID || !system.executable || system.data.length !== 0) throw new Error('Rent or System runtime account is not canonical');
  const marketRent = await client.minimumBalanceForRentExemption(CORE_STATE_BYTES);
  const creditRent = await client.minimumBalanceForRentExemption(LIFECYCLE_RENT_CREDIT_BYTES_V2);
  const marketLamports = finalAccounts.get(market.toBase58())?.lamports ?? '0';
  const marketRentTopUp = BigInt(marketRent.lamports) > BigInt(marketLamports) ? BigInt(marketRent.lamports) - BigInt(marketLamports) : 0n;
  const totalRentDebit = marketRentTopUp + BigInt(creditRent.lamports);
  if (BigInt(payerAccount.lamports) < totalRentDebit) throw new Error('payer cannot cover the exact current Market and lifecycle-credit rent debit');
  const confirmedInfrastructure = await inspectProtocolInfrastructureV1(client, { registryProgram: input.registryProgram, activationCache: input.activationCache });
  if (!infrastructureEqual(infrastructure, confirmedInfrastructure)) throw new Error('immutable infrastructure projection changed during Found construction');
  const blockhash = await client.latestBlockhash(confirmedInfrastructure.observedSlot);
  const rentCreation = compileLifecycleRentCreateTransactionV2({
    payer: input.payer,
    refundWallet: input.refundWallet,
    market: market.toBase58(),
    releaseSet: releaseSetAuthority.digest,
    generation: input.generation,
    rentProgram: infrastructure.rent.program,
    recentBlockhash: blockhash.blockhash,
  });
  if (rentCreation.rentCredit !== rentCredit.toBase58()) throw new Error('lifecycle RentCredit derivation changed during compilation');
  const compiled = compileCoreFoundTransactionV2({
    payer: input.payer,
    coreProgram: infrastructure.core.program,
    market: market.toBase58(),
    generation: input.generation,
    recentBlockhash: blockhash.blockhash,
    accountAddresses,
  });
  return Object.freeze({
    observedSlot: finalObservation.slot,
    market: market.toBase58(),
    rentCredit: rentCredit.toBase58(),
    coreProgram: infrastructure.core.program,
    registryProgram: input.registryProgram,
    rentProgram: infrastructure.rent.program,
    productRecordDigest: hex(product.digest),
    productId: hex(graph.productId),
    outcomeCount: graph.outcomeCount,
    executionReleaseSetId: releaseSetDecoded.id,
    infrastructureProfile: infrastructure.profilePda,
    infrastructureRecognition: infrastructure.recognition,
    marketRentTopUp: marketRentTopUp.toString(),
    rentCreditRentDebit: creditRent.lamports,
    lastValidBlockHeight: blockhash.lastValidBlockHeight,
    accountAddresses: Object.freeze(accountAddresses),
    requiredSigners: compiled.requiredSigners,
    requestBytes: compiled.requestBytes,
    rentCreateRequestBytes: rentCreation.requestBytes,
    rentCreateTransaction: rentCreation.transaction,
    rentCreateWireBytes: rentCreation.wireBytes,
    transaction: compiled.transaction,
    wireBytes: compiled.wireBytes,
  });
}
