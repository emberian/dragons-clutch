import { ascii, hex, requireNonzero, requireZero, slice, u16, u64 } from './bytes';
import {
  CAPABILITY_ENTRY_ACTIVATION_DEADLINE_OFFSET_V1,
  CAPABILITY_ENTRY_ACTIVATION_POLICY_OFFSET_V1,
  CAPABILITY_ENTRY_BYTES_V1,
  CAPABILITY_ENTRY_CAPACITY_PROFILE_ID_OFFSET_V1,
  CAPABILITY_ENTRY_CHILD_DERIVATION_ID_OFFSET_V1,
  CAPABILITY_ENTRY_CHILD_SCHEMA_ID_OFFSET_V1,
  CAPABILITY_ENTRY_CONFIG_ID_OFFSET_V1,
  CAPABILITY_ENTRY_DEPENDENCIES_OFFSET_V1,
  CAPABILITY_ENTRY_DEPENDENCY_COUNT_OFFSET_V1,
  CAPABILITY_ENTRY_KIND_ID_OFFSET_V1,
  CAPABILITY_ENTRY_QUOTE_OFFSET_V1,
  CAPABILITY_ENTRY_RELEASE_ID_OFFSET_V1,
  CAPABILITY_ENTRY_RESERVED_BYTES_V1,
  CAPABILITY_ENTRY_RESERVED_OFFSET_V1,
  CAPABILITY_FUNDING_ALLOCATION_AMOUNT_OFFSET_V1,
  CAPABILITY_FUNDING_ALLOCATION_RESERVED_BYTES_V1,
  CAPABILITY_FUNDING_ALLOCATION_RESERVED_OFFSET_V1,
  CAPABILITY_FUNDING_AMOUNTS_NATIVE_TOTAL_OFFSET_V1,
  CAPABILITY_FUNDING_AMOUNTS_REALM_TOTAL_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_BENEFICIARY_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_BYTES_V1,
  CAPABILITY_FUNDING_BINDING_MINT_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_REALM_ID_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_RELEASE_ID_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_TOKEN_PROGRAM_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_AMOUNTS_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_BINDING_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_COLLATERAL_KIND_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_MAGIC_V1,
  CAPABILITY_FUNDING_QUOTE_RESERVED_BYTES_V1,
  CAPABILITY_FUNDING_QUOTE_RESERVED_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_SCHEMA_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_SCHEMA_VERSION_V1,
  CAPABILITY_MANIFEST_ARTIFACT_PROFILE_V1,
  CAPABILITY_MANIFEST_COUNT_OFFSET_V1,
  CAPABILITY_MANIFEST_HEADER_BYTES_V1,
  CAPABILITY_MANIFEST_HEADER_RESERVED_BYTES_V1,
  CAPABILITY_MANIFEST_MAGIC_OFFSET_V1,
  CAPABILITY_MANIFEST_MAGIC_V1,
  CAPABILITY_MANIFEST_PROFILE_OFFSET_V1,
  CAPABILITY_MANIFEST_RESERVED_OFFSET_V1,
  CAPABILITY_MANIFEST_SCHEMA_OFFSET_V1,
  CAPABILITY_MANIFEST_SCHEMA_VERSION_V1,
  FUNDING_COMPARTMENTS_V1,
  MAX_CAPABILITIES_V1,
  MAX_DEPENDENCIES_PER_CAPABILITY_V1,
} from './generated/capabilityManifestV1';

/**
 * The generated coordinates other browser surfaces name. They are re-exported
 * rather than restated so `lib/generated/capabilityManifestV1.ts` stays the one
 * place any of these numbers is written down.
 */
export {
  CAPABILITY_ENTRY_BYTES_V1,
  CAPABILITY_ENTRY_QUOTE_OFFSET_V1,
  CAPABILITY_FUNDING_ALLOCATION_BYTES_V1,
  CAPABILITY_FUNDING_AMOUNTS_BYTES_V1,
  CAPABILITY_FUNDING_BINDING_BYTES_V1,
  CAPABILITY_FUNDING_QUOTE_BYTES_V1,
  CAPABILITY_FUNDING_QUOTE_MAGIC_V1,
  CAPABILITY_MANIFEST_HEADER_BYTES_V1,
  CAPABILITY_MANIFEST_MAGIC_V1,
  FUNDING_COMPARTMENTS_V1,
  MAX_CAPABILITIES_V1,
} from './generated/capabilityManifestV1';

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

/**
 * Typed capability funding.
 *
 * Native lamports and Realm collateral are two physical dimensions. Nothing
 * here sums or converts across them, and no surface may merge the seven
 * segregated compartments into a single "cost": the manifest keeps them
 * separate and so does this decoder.
 *
 * Every width, offset, magic and compartment coordinate below comes from
 * `lib/generated/capabilityManifestV1.ts`, emitted from the same Lean schema
 * `dclutch-capability-contract` compiles against, so this decoder refuses
 * exactly what the chain refuses.
 */

const MAX_U64 = (BigInt(1) << BigInt(64)) - BigInt(1);
/** Width of one content-addressed identity coordinate. */
const CONTENT_ID_BYTES = 32;

/** The six content identities of one entry, in canonical manifest order. */
const ENTRY_IDENTITY_OFFSETS_V1 = Object.freeze([
  CAPABILITY_ENTRY_KIND_ID_OFFSET_V1,
  CAPABILITY_ENTRY_RELEASE_ID_OFFSET_V1,
  CAPABILITY_ENTRY_CONFIG_ID_OFFSET_V1,
  CAPABILITY_ENTRY_CAPACITY_PROFILE_ID_OFFSET_V1,
  CAPABILITY_ENTRY_CHILD_SCHEMA_ID_OFFSET_V1,
  CAPABILITY_ENTRY_CHILD_DERIVATION_ID_OFFSET_V1,
]);

/** The five identities of a Realm-collateral binding, in canonical order. */
const BINDING_IDENTITY_OFFSETS_V1 = Object.freeze([
  CAPABILITY_FUNDING_BINDING_REALM_ID_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_RELEASE_ID_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_TOKEN_PROGRAM_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_MINT_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_BENEFICIARY_OFFSET_V1,
]);

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
  requireZero(bytes, offset + CAPABILITY_FUNDING_ALLOCATION_RESERVED_OFFSET_V1, CAPABILITY_FUNDING_ALLOCATION_RESERVED_BYTES_V1, `${field} ${compartment.name} compartment`);
  const assetClass = ASSET_CLASSES[bytes[offset]];
  if (assetClass === undefined) throw new Error(`${field} ${compartment.name} compartment names asset class ${bytes[offset]}, which is undefined`);
  const amount = u64(bytes, offset + CAPABILITY_FUNDING_ALLOCATION_AMOUNT_OFFSET_V1);
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
  const parts = BINDING_IDENTITY_OFFSETS_V1.map((relative) => slice(bytes, offset + relative, CONTENT_ID_BYTES));
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
  if (ascii(bytes, offset, 8) !== CAPABILITY_FUNDING_QUOTE_MAGIC_V1) throw new Error(`${field} funding quote magic is not ${CAPABILITY_FUNDING_QUOTE_MAGIC_V1}`);
  const schema = u16(bytes, offset + CAPABILITY_FUNDING_QUOTE_SCHEMA_OFFSET_V1);
  if (schema !== CAPABILITY_FUNDING_QUOTE_SCHEMA_VERSION_V1) throw new Error(`${field} funding quote schema ${schema} is unsupported`);
  requireZero(bytes, offset + CAPABILITY_FUNDING_QUOTE_RESERVED_OFFSET_V1, CAPABILITY_FUNDING_QUOTE_RESERVED_BYTES_V1, `${field} funding quote header`);
  const amountsOffset = offset + CAPABILITY_FUNDING_QUOTE_AMOUNTS_OFFSET_V1;
  const compartments = Object.freeze(FUNDING_COMPARTMENTS_V1.map((_, index) => allocation(bytes, amountsOffset, index, field)));
  const nativeLamportsTotal = checkedTotal(compartments, 'native-lamports', field);
  const realmCollateralTotal = checkedTotal(compartments, 'realm-collateral', field);
  if (nativeLamportsTotal !== u64(bytes, amountsOffset + CAPABILITY_FUNDING_AMOUNTS_NATIVE_TOTAL_OFFSET_V1)
      || realmCollateralTotal !== u64(bytes, amountsOffset + CAPABILITY_FUNDING_AMOUNTS_REALM_TOTAL_OFFSET_V1)) {
    throw new Error(`${field} funding quote asset totals differ from its own typed compartments`);
  }
  const kind = bytes[offset + CAPABILITY_FUNDING_QUOTE_COLLATERAL_KIND_OFFSET_V1];
  let realmCollateral: RealmCollateralBindingV1 | null = null;
  if (kind === 0) {
    requireZero(bytes, offset + CAPABILITY_FUNDING_QUOTE_BINDING_OFFSET_V1, CAPABILITY_FUNDING_BINDING_BYTES_V1, `${field} absent Realm collateral binding`);
  } else if (kind === 1) {
    realmCollateral = collateralBinding(bytes, offset + CAPABILITY_FUNDING_QUOTE_BINDING_OFFSET_V1, field);
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
  if (bytes.length < CAPABILITY_MANIFEST_HEADER_BYTES_V1
      || ascii(bytes, CAPABILITY_MANIFEST_MAGIC_OFFSET_V1, 8) !== CAPABILITY_MANIFEST_MAGIC_V1
      || u16(bytes, CAPABILITY_MANIFEST_SCHEMA_OFFSET_V1) !== CAPABILITY_MANIFEST_SCHEMA_VERSION_V1
      || u16(bytes, CAPABILITY_MANIFEST_PROFILE_OFFSET_V1) !== CAPABILITY_MANIFEST_ARTIFACT_PROFILE_V1) {
    throw new Error('capability manifest has the wrong exact header');
  }
  requireZero(bytes, CAPABILITY_MANIFEST_RESERVED_OFFSET_V1, CAPABILITY_MANIFEST_HEADER_RESERVED_BYTES_V1, 'capability manifest header');
  const count = u16(bytes, CAPABILITY_MANIFEST_COUNT_OFFSET_V1);
  if (count === 0 || count > MAX_CAPABILITIES_V1
      || bytes.length !== CAPABILITY_MANIFEST_HEADER_BYTES_V1 + CAPABILITY_ENTRY_BYTES_V1 * count) {
    throw new Error('capability manifest width is invalid');
  }
  let priorKind: Uint8Array | null = null;
  const entries: CapabilityManifestEntryV1[] = [];
  for (let index = 0; index < count; index += 1) {
    const offset = CAPABILITY_MANIFEST_HEADER_BYTES_V1 + CAPABILITY_ENTRY_BYTES_V1 * index;
    const identities = ENTRY_IDENTITY_OFFSETS_V1.map((relative) => slice(bytes, offset + relative, CONTENT_ID_BYTES));
    identities.forEach((identity, coordinate) => requireNonzero(identity, `capability manifest entry ${index} identity ${coordinate}`));
    if (priorKind !== null) {
      let order = 0;
      while (order < CONTENT_ID_BYTES && priorKind[order] === identities[0][order]) order += 1;
      if (order === CONTENT_ID_BYTES || (priorKind[order] ?? 0) > (identities[0][order] ?? 0)) throw new Error('capability manifest kinds are not strictly ordered');
    }
    priorKind = identities[0];
    requireZero(bytes, offset + CAPABILITY_ENTRY_RESERVED_OFFSET_V1, CAPABILITY_ENTRY_RESERVED_BYTES_V1, `capability manifest entry ${index}`);
    const policy = bytes[offset + CAPABILITY_ENTRY_ACTIVATION_POLICY_OFFSET_V1];
    const deadline = u64(bytes, offset + CAPABILITY_ENTRY_ACTIVATION_DEADLINE_OFFSET_V1);
    if ((policy !== 0 && policy !== 1) || (policy === 0 && deadline !== BigInt(0)) || (policy === 1 && deadline === BigInt(0))) {
      throw new Error('capability manifest activation policy is noncanonical');
    }
    const dependencyCount = bytes[offset + CAPABILITY_ENTRY_DEPENDENCY_COUNT_OFFSET_V1] ?? 0;
    if (dependencyCount > MAX_DEPENDENCIES_PER_CAPABILITY_V1) throw new Error('capability manifest dependency count exceeds its bound');
    const dependencies: number[] = [];
    let priorDependency = -1;
    for (let position = 0; position < MAX_DEPENDENCIES_PER_CAPABILITY_V1; position += 1) {
      const dependency = bytes[offset + CAPABILITY_ENTRY_DEPENDENCIES_OFFSET_V1 + position] ?? 0;
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
      funding: decodeCapabilityFundingQuoteV1(bytes, offset + CAPABILITY_ENTRY_QUOTE_OFFSET_V1, `capability manifest entry ${index}`),
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
