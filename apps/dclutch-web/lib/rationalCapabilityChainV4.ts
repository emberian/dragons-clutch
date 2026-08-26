import { PublicKey } from '@solana/web3.js';

import { ascii, fromHex, isZero, requireNonzero, requireZero, sha256, slice, u16, u64 } from './bytes';
import { decodeCoreFoundProductGraphV2 } from './coreFound';
import {
  CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
  PORTFOLIO_SCHEMA_ID_V2,
  PRODUCT_RECORD_SCHEMA_ID_V2,
  RESULT_DOMAIN_SCHEMA_ID_V2,
} from './generated/coreFound';
import * as Hot from './generated/directInlineV3';
import {
  CAPABILITY_PROGRAM_V4_SCHEMA,
  RATIONAL_REPRESENTATION_DESCRIPTOR_SCHEMA_V3,
  acquireRationalHotAccountsV4,
  authenticateFinalizedRationalHotRecordV4,
  authenticateRationalHotActivationV4,
  decodeRationalHotCapabilityV4,
  decodeRationalHotCoreV2,
  decodeRationalHotLookupTableV4,
  decodeRationalHotRootV4,
  decodeRationalRepresentationDescriptorV3,
  rationalHotFixedMetasV4,
  selectRationalHotCapabilityV4,
  selectRationalHotManifestEntryV4,
  type RationalHotAccountMetaV4,
  type RationalHotCapabilityV4,
  type RationalHotCoreViewV2,
  type RationalHotRpcV4,
} from './rationalRetireReceiptV4';
import {
  TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
  decodeTokenBehaviorSelectionV2,
} from './rationalTokenV2';
import { deriveFinalizedRecordAddressesV1, RENT_SYSVAR_ID, SYSTEM_PROGRAM_ID } from './releaseRegistry';
import { type RpcAccount } from './rpc';

const PROGRAM_SET_SCHEMA_V2 = Uint8Array.from([
  0x37,0xdf,0x09,0xe7,0xde,0xeb,0xdd,0x0a,0xd0,0xd1,0x25,0x13,0xa7,0x8d,0xd4,0x4c,
  0x97,0x24,0x30,0x37,0x99,0xb7,0x54,0x4d,0xc9,0x1b,0x3b,0x6a,0x2e,0x6d,0x62,0x96,
]);
const REPRESENTATION_GRAPH_SCHEMA_V2 = Uint8Array.from([
  0xbe,0x69,0x36,0xbb,0xa2,0x4e,0xa0,0xd2,0xd1,0x78,0xfa,0x65,0x92,0x74,0x8e,0xa5,
  0xf5,0xdc,0x95,0xdf,0x9a,0x72,0xbb,0xa8,0x58,0x84,0xa9,0x27,0xe2,0x89,0xd5,0x97,
]);
const INSTRUCTIONS_SYSVAR = 'Sysvar1nstructions1111111111111111111111111';

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function key(value: string, field: string): PublicKey {
  const parsed = new PublicKey(value);
  if (parsed.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  return parsed;
}

function required(accounts: ReadonlyMap<string, RpcAccount | null>, address: string, field: string): RpcAccount {
  const account = accounts.get(address);
  if (account === null || account === undefined) throw new Error(`${field} is absent at finalized commitment`);
  return account;
}

function readU32(bytes: Uint8Array, offset: number): number {
  const value = slice(bytes, offset, 4);
  return new DataView(value.buffer, value.byteOffset, value.byteLength).getUint32(0, true);
}

export function authenticateRationalRepresentationGraphV2(
  bytes: Uint8Array,
  descriptor: ReturnType<typeof decodeRationalRepresentationDescriptorV3>,
): void {
  if (bytes.length < 104 || ascii(bytes, 0, 8) !== 'DCRRGRP2' || u16(bytes, 8) !== 2) throw new Error('representation graph has the wrong exact V2 header');
  requireZero(bytes, 10, 6, 'representation graph header');
  const outcomes = readU32(bytes, 80); const nodes = readU32(bytes, 84); const edges = readU32(bytes, 88);
  requireZero(bytes, 92, 4, 'representation graph header');
  const exact = 104 + nodes * 64 + edges * 48 + nodes * outcomes * 8;
  if (outcomes !== descriptor.outcomeCount || nodes === 0 || !Number.isSafeInteger(exact) || bytes.length !== exact
      || !same(slice(bytes, 16, 32), descriptor.graphId) || !same(slice(bytes, 48, 32), descriptor.rootId)) {
    throw new Error('representation graph width or descriptor join is inconsistent');
  }
  const scale = u64(bytes, 96); if (scale === 0n) throw new Error('representation graph scale is zero');
  const rootId = slice(bytes, 48, 32); let root = -1;
  for (let index = 0; index < nodes; index += 1) {
    if (same(slice(bytes, 104 + index * 64, 32), rootId)) {
      if (root !== -1) throw new Error('representation graph repeats its root identity');
      root = index;
    }
  }
  if (root < 0) throw new Error('representation graph omits its selected root');
  const exposure = 104 + nodes * 64 + edges * 48 + root * outcomes * 8;
  const coefficients = new Map(descriptor.support.map(({ outcome, coefficient }) => [outcome, coefficient]));
  for (let outcome = 0; outcome < outcomes; outcome += 1) {
    const coefficient = coefficients.get(outcome) ?? 0n;
    if (coefficient * scale !== u64(bytes, exposure + outcome * 8) * descriptor.denominator) {
      throw new Error(`representation descriptor payoff differs from graph root at outcome ${outcome}`);
    }
  }
}

export type RationalCapabilityPhaseV4 = 'open' | 'terminal';

export type RationalCapabilityCommonV4 = Readonly<{
  observedSlot: string;
  accounts: ReadonlyMap<string, RpcAccount | null>;
  fixed: ReadonlyArray<RationalHotAccountMetaV4>;
  payer: string;
  actor: string;
  lookupTable: ReturnType<typeof decodeRationalHotLookupTableV4>;
  marketAddress: string;
  coreProgram: string;
  trading: string;
  registry: string;
  market: RationalHotCoreViewV2;
  activation: Awaited<ReturnType<typeof authenticateRationalHotActivationV4>>;
  capabilitySelection: Readonly<{ schema: Uint8Array; digest: Uint8Array }>;
  capability: RationalHotCapabilityV4;
  configDigest: Uint8Array;
  rootDigest: Uint8Array;
  artifacts: ReadonlyArray<RpcAccount>;
  descriptorId: Uint8Array;
  descriptorAddresses: Readonly<{ record: string; staging: string }>;
  descriptor: ReturnType<typeof decodeRationalRepresentationDescriptorV3>;
  graphAddresses: Readonly<{ record: string; staging: string }>;
  productRaw: RpcAccount;
  domainRaw: RpcAccount;
  portfolioRaw: RpcAccount;
  domainDigest: Uint8Array;
  portfolioDigest: Uint8Array;
  product: ReturnType<typeof decodeCoreFoundProductGraphV2>;
}>;

/** One semantic owner for the common finalized CapabilityV4/Product route. */
export async function inspectRationalCapabilityCommonV4(
  client: RationalHotRpcV4,
  input: Readonly<{
    phase: RationalCapabilityPhaseV4;
    selector: number;
    requestSchema: Uint8Array;
    payer: string;
    actor: string;
    fixedAccounts: ReadonlyArray<string>;
    lookupTable: string;
    descriptorId: string;
  }>,
): Promise<RationalCapabilityCommonV4> {
  const payer = key(input.payer, 'payer').toBase58(); const actor = key(input.actor, 'actor').toBase58();
  const fixed = rationalHotFixedMetasV4(input.fixedAccounts);
  const first = await acquireRationalHotAccountsV4(client, [...fixed.map((meta) => meta.address), payer, actor, input.lookupTable]);
  const marketAddress = fixed[Hot.HOT_MARKET_ACCOUNT_V3]?.address ?? '';
  const coreProgram = fixed[Hot.HOT_CORE_PROGRAM_ACCOUNT_V3]?.address ?? '';
  const trading = fixed[Hot.HOT_TRADING_PROGRAM_ACCOUNT_V3]?.address ?? '';
  const registry = fixed[Hot.HOT_REGISTRY_PROGRAM_ACCOUNT_V3]?.address ?? '';
  const marketAccount = required(first.accounts, marketAddress, 'Core Market');
  const market = decodeRationalHotCoreV2(marketAddress, marketAccount, coreProgram);
  const expectedPhase = input.phase === 'open' ? 1 : 2;
  if (market.phase !== expectedPhase || market.readiness !== 2
      || (input.phase === 'open' ? !isZero(market.terminalReceipt) : isZero(market.terminalReceipt))) {
    throw new Error(`Rational ${input.phase} requires the exact readiness-consumed ${input.phase === 'open' ? 'Open' : 'Terminal'} Core lifecycle`);
  }
  if (market.registry !== registry || fixed[Hot.HOT_RENT_SYSVAR_ACCOUNT_V3]?.address !== RENT_SYSVAR_ID
      || fixed[Hot.HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3]?.address !== INSTRUCTIONS_SYSVAR) {
    throw new Error('Hot fixed frame differs from Core or runtime sysvars');
  }
  for (const [account, field] of [[required(first.accounts, payer, 'payer'), 'payer'], [required(first.accounts, actor, 'actor'), 'actor']] as const) {
    if (account.owner !== SYSTEM_PROGRAM_ID || account.executable || account.data.length !== 0) throw new Error(`${field} is not a System-owned data-free wallet`);
  }
  const rootAddress = fixed[Hot.HOT_ROOT_ACCOUNT_V3]?.address ?? '';
  const rootAccount = required(first.accounts, rootAddress, 'capability root');
  if (rootAccount.owner !== trading || rootAccount.executable) throw new Error('capability root is not Trading-owned state');
  const selection = decodeRationalHotRootV4(rootAccount.data, market.releaseSet, marketAddress, market.generation);
  if (!same(selection.manifest, market.manifest)) throw new Error('capability root and Core select different manifests');
  const activation = await authenticateRationalHotActivationV4(
    required(first.accounts, fixed[Hot.HOT_ACTIVATION_CACHE_ACCOUNT_V3]?.address ?? '', 'activation cache'),
    fixed[Hot.HOT_ACTIVATION_CACHE_ACCOUNT_V3]?.address ?? '', registry, market.releaseSet, coreProgram,
    fixed[Hot.HOT_CORE_PROGRAMDATA_ACCOUNT_V3]?.address ?? '', trading, fixed[Hot.HOT_TRADING_PROGRAMDATA_ACCOUNT_V3]?.address ?? '',
  );
  const manifestRaw = await authenticateFinalizedRationalHotRecordV4(client, first.accounts, registry,
    fixed[Hot.HOT_MANIFEST_RAW_ACCOUNT_V3]?.address ?? '', fixed[Hot.HOT_MANIFEST_STAGING_ACCOUNT_V3]?.address ?? '',
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, selection.manifest, 'capability manifest');
  const manifest = selectRationalHotManifestEntryV4(manifestRaw.data, selection);
  const setRaw = await authenticateFinalizedRationalHotRecordV4(client, first.accounts, registry,
    fixed[Hot.HOT_PROGRAM_SET_RAW_ACCOUNT_V3]?.address ?? '', fixed[Hot.HOT_PROGRAM_SET_STAGING_ACCOUNT_V3]?.address ?? '',
    PROGRAM_SET_SCHEMA_V2, selection.programSet, 'Rational ProgramSetV2');
  const capabilitySelection = selectRationalHotCapabilityV4(setRaw.data, input.selector);
  if (!same(capabilitySelection.schema, CAPABILITY_PROGRAM_V4_SCHEMA)) throw new Error('Rational selector does not choose CapabilityProgramV4');
  const capabilityRaw = await authenticateFinalizedRationalHotRecordV4(client, first.accounts, registry,
    fixed[Hot.HOT_DESCRIPTOR_RAW_ACCOUNT_V3]?.address ?? '', fixed[Hot.HOT_DESCRIPTOR_STAGING_ACCOUNT_V3]?.address ?? '',
    capabilitySelection.schema, capabilitySelection.digest, 'Rational CapabilityProgramV4');
  const capability = decodeRationalHotCapabilityV4(capabilityRaw.data);
  if (!same(capability.kind, selection.kind) || !same(capability.configSchema, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2)
      || !same(capability.requestSchema, input.requestSchema) || !same(capability.rootSchema, manifest.rootSchema)
      || !same(capability.derivation, manifest.derivation) || !same(capability.capacity, manifest.capacity)
      || rootAccount.data.length !== 232 + capability.rootStateBytes) {
    throw new Error('Rational CapabilityV4 does not join the selected manifest/root/config/request family');
  }
  const configRaw = await authenticateFinalizedRationalHotRecordV4(client, first.accounts, registry,
    fixed[Hot.HOT_CONFIG_RAW_ACCOUNT_V3]?.address ?? '', fixed[Hot.HOT_CONFIG_STAGING_ACCOUNT_V3]?.address ?? '',
    capability.configSchema, selection.config, 'TokenBehaviorSelectionV2');
  decodeTokenBehaviorSelectionV2(configRaw.data, market.realm, market.releaseSet);
  const rawIndexes = [10, 12, 18, 20, 14, 16]; const artifacts: RpcAccount[] = [];
  for (let index = 0; index < capability.artifacts.length; index += 1) {
    const raw = rawIndexes[index] ?? -1; const artifact = capability.artifacts[index];
    if (raw < 0 || artifact === undefined) throw new Error('CapabilityV4 artifact table is incomplete');
    artifacts.push(await authenticateFinalizedRationalHotRecordV4(client, first.accounts, registry,
      fixed[raw]?.address ?? '', fixed[raw + 1]?.address ?? '', artifact.schema, artifact.digest, `Rational artifact ${index}`));
  }
  const descriptorId = fromHex(input.descriptorId, 'representation descriptor identity'); requireNonzero(descriptorId, 'representation descriptor');
  const descriptorAddresses = deriveFinalizedRecordAddressesV1(registry, RATIONAL_REPRESENTATION_DESCRIPTOR_SCHEMA_V3, descriptorId);
  const descriptorObservation = await acquireRationalHotAccountsV4(client, [descriptorAddresses.record, descriptorAddresses.staging], first.slot);
  const descriptorRaw = await authenticateFinalizedRationalHotRecordV4(client, descriptorObservation.accounts, registry,
    descriptorAddresses.record, descriptorAddresses.staging, RATIONAL_REPRESENTATION_DESCRIPTOR_SCHEMA_V3, descriptorId, 'representation descriptor');
  const descriptor = decodeRationalRepresentationDescriptorV3(descriptorRaw.data, descriptorId);
  if (descriptor.market !== marketAddress || !same(descriptor.releaseSet, market.releaseSet)) throw new Error('representation descriptor differs from Market/release');
  const graphAddresses = deriveFinalizedRecordAddressesV1(registry, REPRESENTATION_GRAPH_SCHEMA_V2, descriptor.graphDigest);
  const graphObservation = await acquireRationalHotAccountsV4(client, [graphAddresses.record, graphAddresses.staging], descriptorObservation.slot);
  const graphRaw = await authenticateFinalizedRationalHotRecordV4(client, graphObservation.accounts, registry,
    graphAddresses.record, graphAddresses.staging, REPRESENTATION_GRAPH_SCHEMA_V2, descriptor.graphDigest, 'representation graph');
  authenticateRationalRepresentationGraphV2(graphRaw.data, descriptor);
  const merged = new Map([...first.accounts, ...descriptorObservation.accounts, ...graphObservation.accounts]);
  const productRaw = await authenticateFinalizedRationalHotRecordV4(client, merged, registry,
    fixed[Hot.HOT_PRODUCT_RAW_ACCOUNT_V3]?.address ?? '', fixed[Hot.HOT_PRODUCT_STAGING_ACCOUNT_V3]?.address ?? '', PRODUCT_RECORD_SCHEMA_ID_V2, market.productRecord, 'Product Runtime V2 root');
  const domainDigest = slice(productRaw.data, 48, 32); const portfolioDigest = slice(productRaw.data, 80, 32);
  const domainRaw = await authenticateFinalizedRationalHotRecordV4(client, merged, registry,
    fixed[Hot.HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3]?.address ?? '', fixed[Hot.HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3]?.address ?? '', RESULT_DOMAIN_SCHEMA_ID_V2, domainDigest, 'Product result domain');
  const portfolioRaw = await authenticateFinalizedRationalHotRecordV4(client, merged, registry,
    fixed[Hot.HOT_PORTFOLIO_RAW_ACCOUNT_V3]?.address ?? '', fixed[Hot.HOT_PORTFOLIO_STAGING_ACCOUNT_V3]?.address ?? '', PORTFOLIO_SCHEMA_ID_V2, portfolioDigest, 'Product portfolio');
  const product = decodeCoreFoundProductGraphV2(productRaw.data, domainRaw.data, portfolioRaw.data, domainDigest, portfolioDigest);
  if (!same(product.productId, market.productId)) throw new Error('Product identity differs from the Core Market');
  const lookupTable = decodeRationalHotLookupTableV4(input.lookupTable, required(merged, input.lookupTable, 'address lookup table'));
  return Object.freeze({ observedSlot: graphObservation.slot, accounts: merged, fixed, payer, actor, lookupTable,
    marketAddress, coreProgram, trading, registry, market, activation, capabilitySelection, capability,
    configDigest: selection.config, rootDigest: await sha256(rootAccount.data), artifacts: Object.freeze(artifacts),
    descriptorId, descriptorAddresses, descriptor, graphAddresses, productRaw, domainRaw, portfolioRaw,
    domainDigest, portfolioDigest, product });
}
