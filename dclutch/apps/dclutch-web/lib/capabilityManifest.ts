import { ascii, hex, requireNonzero, requireZero, slice, u16, u64 } from './bytes';

/**
 * The immutable `DCLTCAP1` capability manifest a Market root commits to by
 * content identity.
 *
 * A Market names capability children only through this manifest; nothing else
 * in the browser may assert that a Market "has" a capability. Every entry is
 * decoded exhaustively here — ordering, activation policy, the dependency
 * list, and the immutable typed funding quote included — so a caller cannot
 * select an entry out of a manifest whose remaining entries were never
 * checked, and cannot be shown a manifest the canonical contract would refuse.
 */

export const CAPABILITY_MANIFEST_MAGIC = 'DCLTCAP1';
export const CAPABILITY_MANIFEST_HEADER_BYTES = 16;
export const CAPABILITY_MANIFEST_ENTRY_BYTES = 528;
export const CAPABILITY_MANIFEST_MAX_ENTRIES = 16;

/**
 * Typed capability funding.
 *
 * Native lamports and Realm collateral are two physical dimensions. Nothing
 * here sums or converts across them, and no surface may merge the seven
 * segregated compartments into a single "cost": the manifest keeps them
 * separate and so does this decoder.
 */

export const CAPABILITY_FUNDING_QUOTE_OFFSET = 224;
export const CAPABILITY_FUNDING_QUOTE_BYTES = 304;
export const FUNDING_AMOUNTS_BYTES = 128;
export const FUNDING_ALLOCATION_BYTES = 16;
export const FUNDING_QUOTE_MAGIC = 'DCLTFQ01';
export const REALM_COLLATERAL_BINDING_BYTES = 160;

const QUOTE_COLLATERAL_KIND_OFFSET = 10;
const QUOTE_RESERVED_OFFSET = 11;
const QUOTE_RESERVED_BYTES = 5;
const QUOTE_BINDING_OFFSET = 16;
const QUOTE_AMOUNTS_OFFSET = QUOTE_BINDING_OFFSET + REALM_COLLATERAL_BINDING_BYTES;
const ALLOCATION_RESERVED_OFFSET = 1;
const ALLOCATION_RESERVED_BYTES = 7;
const ALLOCATION_AMOUNT_OFFSET = 8;
const AMOUNTS_NATIVE_TOTAL_OFFSET = 112;
const AMOUNTS_REALM_TOTAL_OFFSET = 120;
const MAX_U64 = (BigInt(1) << BigInt(64)) - BigInt(1);

/**
 * The seven segregated compartments, in their canonical manifest order.
 *
 * `Rent` and `Creation` are intrinsically native lamports. The remaining five
 * carry whichever asset class the immutable capability quote selected.
 */
export const FUNDING_COMPARTMENTS_V1 = Object.freeze([
  Object.freeze({ name: 'Rent', offset: 0, assetPolicy: 'native-lamports-only' }),
  Object.freeze({ name: 'Creation', offset: 16, assetPolicy: 'native-lamports-only' }),
  Object.freeze({ name: 'Work', offset: 32, assetPolicy: 'capability-selected' }),
  Object.freeze({ name: 'Provider', offset: 48, assetPolicy: 'capability-selected' }),
  Object.freeze({ name: 'Bounty', offset: 64, assetPolicy: 'capability-selected' }),
  Object.freeze({ name: 'Liquidity', offset: 80, assetPolicy: 'capability-selected' }),
  Object.freeze({ name: 'Service', offset: 96, assetPolicy: 'capability-selected' }),
] as const);

export type FundingCompartmentNameV1 = (typeof FUNDING_COMPARTMENTS_V1)[number]['name'];
export type FundingAssetPolicyV1 = (typeof FUNDING_COMPARTMENTS_V1)[number]['assetPolicy'];
export type FundingAssetClassV1 = 'not-applicable' | 'native-lamports' | 'realm-collateral';

const ASSET_CLASSES: ReadonlyArray<FundingAssetClassV1> = Object.freeze(['not-applicable', 'native-lamports', 'realm-collateral']);

export type CompartmentFundingV1 = Readonly<{
  compartment: FundingCompartmentNameV1;
  assetPolicy: FundingAssetPolicyV1;
  assetClass: FundingAssetClassV1;
  amount: bigint;
}>;

export type RealmCollateralBindingV1 = Readonly<{
  realmId: Uint8Array;
  collateralReleaseId: Uint8Array;
  tokenProgram: Uint8Array;
  mint: Uint8Array;
  refundTokenBeneficiary: Uint8Array;
}>;

export type CapabilityFundingQuoteV1 = Readonly<{
  compartments: ReadonlyArray<CompartmentFundingV1>;
  nativeLamportsTotal: bigint;
  realmCollateralTotal: bigint;
  realmCollateral: RealmCollateralBindingV1 | null;
}>;

function allocation(bytes: Uint8Array, base: number, index: number, field: string): CompartmentFundingV1 {
  const compartment = FUNDING_COMPARTMENTS_V1[index];
  const offset = base + compartment.offset;
  requireZero(bytes, offset + ALLOCATION_RESERVED_OFFSET, ALLOCATION_RESERVED_BYTES, `${field} ${compartment.name} compartment`);
  const assetClass = ASSET_CLASSES[bytes[offset]];
  if (assetClass === undefined) throw new Error(`${field} ${compartment.name} compartment names asset class ${bytes[offset]}, which is undefined`);
  const amount = u64(bytes, offset + ALLOCATION_AMOUNT_OFFSET);
  if ((assetClass === 'not-applicable') !== (amount === BigInt(0))) {
    throw new Error(`${field} ${compartment.name} compartment is not one canonical typed amount`);
  }
  if (compartment.assetPolicy === 'native-lamports-only' && assetClass === 'realm-collateral') {
    throw new Error(`${field} ${compartment.name} compartment is intrinsically native lamports and cannot name Realm collateral`);
  }
  return Object.freeze({ compartment: compartment.name, assetPolicy: compartment.assetPolicy, assetClass, amount });
}

function checkedTotal(compartments: ReadonlyArray<CompartmentFundingV1>, assetClass: FundingAssetClassV1, field: string): bigint {
  let total = BigInt(0);
  for (const entry of compartments) {
    if (entry.assetClass !== assetClass) continue;
    total += entry.amount;
    if (total > MAX_U64) throw new Error(`${field} ${assetClass} compartments sum above the exact u64 bound`);
  }
  return total;
}

function collateralBinding(bytes: Uint8Array, offset: number, field: string): RealmCollateralBindingV1 {
  const parts = [0, 32, 64, 96, 128].map((relative) => slice(bytes, offset + relative, 32));
  const names = ['Realm identity', 'collateral release identity', 'token program', 'collateral mint', 'refund token beneficiary'] as const;
  parts.forEach((part, index) => requireNonzero(part, `${field} Realm collateral ${names[index]}`));
  return Object.freeze({
    realmId: parts[0],
    collateralReleaseId: parts[1],
    tokenProgram: parts[2],
    mint: parts[3],
    refundTokenBeneficiary: parts[4],
  });
}

/**
 * Decode one immutable `DCLTFQ01` typed funding quote at an exact offset.
 *
 * Every refusal the canonical contract makes is made here: unknown asset
 * class, a compartment whose class and amount disagree, a checked total that
 * differs from its own compartments, and a Realm-collateral binding that is
 * present without collateral or absent with it.
 */
export function decodeCapabilityFundingQuoteV1(bytes: Uint8Array, offset: number, field: string): CapabilityFundingQuoteV1 {
  if (ascii(bytes, offset, 8) !== FUNDING_QUOTE_MAGIC) throw new Error(`${field} funding quote magic is not ${FUNDING_QUOTE_MAGIC}`);
  if (u16(bytes, offset + 8) !== 1) throw new Error(`${field} funding quote schema ${u16(bytes, offset + 8)} is unsupported`);
  requireZero(bytes, offset + QUOTE_RESERVED_OFFSET, QUOTE_RESERVED_BYTES, `${field} funding quote header`);
  const amountsOffset = offset + QUOTE_AMOUNTS_OFFSET;
  const compartments = Object.freeze(FUNDING_COMPARTMENTS_V1.map((_, index) => allocation(bytes, amountsOffset, index, field)));
  const nativeLamportsTotal = checkedTotal(compartments, 'native-lamports', field);
  const realmCollateralTotal = checkedTotal(compartments, 'realm-collateral', field);
  if (nativeLamportsTotal !== u64(bytes, amountsOffset + AMOUNTS_NATIVE_TOTAL_OFFSET)
      || realmCollateralTotal !== u64(bytes, amountsOffset + AMOUNTS_REALM_TOTAL_OFFSET)) {
    throw new Error(`${field} funding quote asset totals differ from its own typed compartments`);
  }
  const kind = bytes[offset + QUOTE_COLLATERAL_KIND_OFFSET];
  let realmCollateral: RealmCollateralBindingV1 | null = null;
  if (kind === 0) {
    requireZero(bytes, offset + QUOTE_BINDING_OFFSET, REALM_COLLATERAL_BINDING_BYTES, `${field} absent Realm collateral binding`);
  } else if (kind === 1) {
    realmCollateral = collateralBinding(bytes, offset + QUOTE_BINDING_OFFSET, field);
  } else {
    throw new Error(`${field} funding quote collateral kind ${kind} is undefined`);
  }
  if ((realmCollateralTotal === BigInt(0)) !== (realmCollateral === null)) {
    throw new Error(`${field} funding quote Realm collateral binding does not match its own collateral total`);
  }
  return Object.freeze({ compartments, nativeLamportsTotal, realmCollateralTotal, realmCollateral });
}

export type CapabilityActivationV1 = 'immediate' | 'deadline';

export type CapabilityManifestEntryV1 = Readonly<{
  index: number;
  kind: Uint8Array;
  programSet: Uint8Array;
  config: Uint8Array;
  capacity: Uint8Array;
  rootSchema: Uint8Array;
  derivation: Uint8Array;
  activation: CapabilityActivationV1;
  deadline: bigint;
  dependencies: ReadonlyArray<number>;
  funding: CapabilityFundingQuoteV1;
}>;

/** Decode and fully validate every entry of one capability manifest. */
export function decodeCapabilityManifestV1(bytes: Uint8Array): ReadonlyArray<CapabilityManifestEntryV1> {
  if (bytes.length < CAPABILITY_MANIFEST_HEADER_BYTES
      || ascii(bytes, 0, 8) !== CAPABILITY_MANIFEST_MAGIC
      || u16(bytes, 8) !== 1
      || u16(bytes, 10) !== 1) {
    throw new Error('capability manifest has the wrong exact header');
  }
  requireZero(bytes, 14, 2, 'capability manifest header');
  const count = u16(bytes, 12);
  if (count === 0 || count > CAPABILITY_MANIFEST_MAX_ENTRIES
      || bytes.length !== CAPABILITY_MANIFEST_HEADER_BYTES + CAPABILITY_MANIFEST_ENTRY_BYTES * count) {
    throw new Error('capability manifest width is invalid');
  }
  let priorKind: Uint8Array | null = null;
  const entries: CapabilityManifestEntryV1[] = [];
  for (let index = 0; index < count; index += 1) {
    const offset = CAPABILITY_MANIFEST_HEADER_BYTES + CAPABILITY_MANIFEST_ENTRY_BYTES * index;
    const identities = [0, 32, 64, 96, 128, 160].map((relative) => slice(bytes, offset + relative, 32));
    identities.forEach((identity, coordinate) => requireNonzero(identity, `capability manifest entry ${index} identity ${coordinate}`));
    if (priorKind !== null) {
      let order = 0;
      while (order < 32 && priorKind[order] === identities[0][order]) order += 1;
      if (order === 32 || (priorKind[order] ?? 0) > (identities[0][order] ?? 0)) throw new Error('capability manifest kinds are not strictly ordered');
    }
    priorKind = identities[0];
    requireZero(bytes, offset + 194, 6, `capability manifest entry ${index}`);
    const policy = bytes[offset + 192];
    const deadline = u64(bytes, offset + 200);
    if ((policy !== 0 && policy !== 1) || (policy === 0 && deadline !== BigInt(0)) || (policy === 1 && deadline === BigInt(0))) {
      throw new Error('capability manifest activation policy is noncanonical');
    }
    const dependencyCount = bytes[offset + 193] ?? 0;
    if (dependencyCount > CAPABILITY_MANIFEST_MAX_ENTRIES) throw new Error('capability manifest dependency count exceeds its bound');
    const dependencies: number[] = [];
    let priorDependency = -1;
    for (let position = 0; position < CAPABILITY_MANIFEST_MAX_ENTRIES; position += 1) {
      const dependency = bytes[offset + 208 + position] ?? 0;
      if (position < dependencyCount) {
        if (dependency >= count || dependency === index || dependency <= priorDependency) throw new Error('capability manifest dependency list is noncanonical');
        priorDependency = dependency;
        dependencies.push(dependency);
      } else if (dependency !== 0) {
        throw new Error('capability manifest inactive dependency is nonzero');
      }
    }
    entries.push(Object.freeze({
      index,
      kind: identities[0],
      programSet: identities[1],
      config: identities[2],
      capacity: identities[3],
      rootSchema: identities[4],
      derivation: identities[5],
      activation: policy === 0 ? 'immediate' : 'deadline',
      deadline,
      dependencies: Object.freeze(dependencies),
      funding: decodeCapabilityFundingQuoteV1(bytes, offset + CAPABILITY_FUNDING_QUOTE_OFFSET, `capability manifest entry ${index}`),
    }));
  }
  return Object.freeze(entries);
}

/**
 * Kinds this browser can name. A kind absent from this table is still listed
 * from the authenticated manifest; it is labelled as unrecognized rather than
 * given an invented meaning.
 */
export const RECOGNIZED_CAPABILITY_KINDS_V1: Readonly<Record<string, string>> = Object.freeze({
  '8e8a063932339a7eb910608e76b1e70ad0f41b999b6252eeab890ffb733b5474': 'Product payoff admission',
});

export function recognizeCapabilityKindV1(kind: Uint8Array): string | null {
  return RECOGNIZED_CAPABILITY_KINDS_V1[hex(kind)] ?? null;
}
