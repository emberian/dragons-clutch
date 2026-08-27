/**
 * Composing a `CapabilityManifestV1` and its seven-compartment funding quotes.
 *
 * `lib/coreFound.ts` already *validates* a manifest the chain holds. A create
 * wizard needs the other direction: an operator states what a Market's
 * capabilities cost, and the browser has to turn that into the exact bytes a
 * Registry record will be addressed by. The digest of these bytes is the
 * manifest identity that goes into the Market PDA, so this encoder is not a
 * convenience — a byte wrong here is a different Market.
 *
 * WHY THE SEVEN COMPARTMENTS ARE NOT ONE NUMBER. Funding is quoted per
 * compartment and carries two independent checked totals, one for native
 * lamports and one for Realm collateral. Nothing anywhere in the tree adds a
 * lamport to a collateral atom, and the encoder below cannot express it: the
 * totals are recomputed from the compartments and the presence of a Realm
 * collateral binding is a biconditional on the Realm total being nonzero.
 * `Rent` and `Creation` are additionally native-only, because they pay for
 * account existence rather than for work.
 *
 * Every offset and width comes from `lib/generated/capabilityManifestV1.ts`,
 * emitted by `formal/dclutch-semantics/EmitCapabilityManifestV1AbiTs.lean` from
 * the same schema that emits the Rust. The result is round-tripped through
 * `validateCoreFoundCapabilityManifestV1` — the browser's strictest reader,
 * the one the Found path already uses — so nothing this module builds can be
 * something that decoder would refuse.
 */

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
  CAPABILITY_FUNDING_ALLOCATION_AMOUNT_OFFSET_V1,
  CAPABILITY_FUNDING_ALLOCATION_CLASS_OFFSET_V1,
  CAPABILITY_FUNDING_AMOUNTS_NATIVE_TOTAL_OFFSET_V1,
  CAPABILITY_FUNDING_AMOUNTS_REALM_TOTAL_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_BENEFICIARY_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_MINT_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_REALM_ID_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_RELEASE_ID_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_TOKEN_PROGRAM_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_AMOUNTS_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_BINDING_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_BYTES_V1,
  CAPABILITY_FUNDING_QUOTE_COLLATERAL_KIND_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_MAGIC_V1,
  CAPABILITY_FUNDING_QUOTE_SCHEMA_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_SCHEMA_VERSION_V1,
  CAPABILITY_MANIFEST_ARTIFACT_PROFILE_V1,
  CAPABILITY_MANIFEST_COUNT_OFFSET_V1,
  CAPABILITY_MANIFEST_HEADER_BYTES_V1,
  CAPABILITY_MANIFEST_MAGIC_V1,
  CAPABILITY_MANIFEST_PROFILE_OFFSET_V1,
  CAPABILITY_MANIFEST_SCHEMA_OFFSET_V1,
  CAPABILITY_MANIFEST_SCHEMA_VERSION_V1,
  FUNDING_COMPARTMENTS_V1,
  MAX_CAPABILITIES_V1,
  MAX_DEPENDENCIES_PER_CAPABILITY_V1,
} from '../generated/capabilityManifestV1';
import { validateCoreFoundCapabilityManifestV1 } from '../coreFound';

const MAX_U64 = 0xffff_ffff_ffff_ffffn;

export type FundingCompartmentNameV1 = (typeof FUNDING_COMPARTMENTS_V1)[number]['name'];

/** The wire discriminants of `FundingAssetClassV1`, by their array position. */
export const FUNDING_ASSET_CLASSES_V1 = Object.freeze(['not-applicable', 'native-lamports', 'realm-collateral'] as const);
export type FundingAssetClassV1 = (typeof FUNDING_ASSET_CLASSES_V1)[number];

export type CompartmentFundingV1 = Readonly<{ assetClass: FundingAssetClassV1; amount: bigint }>;

export type RealmCollateralBindingV1 = Readonly<{
  realmId: string;
  collateralReleaseId: string;
  tokenProgram: string;
  mint: string;
  refundTokenBeneficiary: string;
}>;

export type FundingQuoteInputV1 = Readonly<{
  compartments: Readonly<Partial<Record<FundingCompartmentNameV1, CompartmentFundingV1>>>;
  realmCollateral: RealmCollateralBindingV1 | null;
}>;

export type CapabilityEntryInputV1 = Readonly<{
  kindId: string;
  releaseId: string;
  configId: string;
  capacityProfileId: string;
  childSchemaId: string;
  childDerivationId: string;
  activation: 'RequiredAtFounding' | 'PrepaidLazy';
  activationDeadlineSlot: bigint;
  dependencies: ReadonlyArray<number>;
  quote: FundingQuoteInputV1;
}>;

/** A compartment nobody funds: class `NotApplicable`, amount exactly zero. */
export const NOT_APPLICABLE_V1: CompartmentFundingV1 = Object.freeze({ assetClass: 'not-applicable', amount: 0n });

/** Native lamports. Refuses zero, because class and amount state one fact. */
export function nativeLamportsV1(amount: bigint): CompartmentFundingV1 {
  if (typeof amount !== 'bigint' || amount <= 0n || amount > MAX_U64) {
    throw new Error('a native-lamports compartment must carry a nonzero u64 amount');
  }
  return Object.freeze({ assetClass: 'native-lamports', amount });
}

/** Realm collateral. Only the five capability-selected compartments admit it. */
export function realmCollateralV1(amount: bigint): CompartmentFundingV1 {
  if (typeof amount !== 'bigint' || amount <= 0n || amount > MAX_U64) {
    throw new Error('a Realm-collateral compartment must carry a nonzero u64 amount');
  }
  return Object.freeze({ assetClass: 'realm-collateral', amount });
}

function identity(value: string, field: string): Uint8Array {
  if (!/^[0-9a-f]{64}$/.test(value)) throw new Error(`${field} must be exactly 32 lowercase hexadecimal bytes`);
  const bytes = Uint8Array.from(value.match(/../g) ?? [], (byte) => Number.parseInt(byte, 16));
  if (bytes.every((byte) => byte === 0)) throw new Error(`${field} is the reserved all-zero identity`);
  return bytes;
}

/**
 * Encode one 304-byte `FundingQuoteV1`.
 *
 * The two totals are recomputed here rather than taken from the caller. A
 * caller-supplied total is a second statement of the same fact, and the reader
 * refuses when the two disagree — so accepting one would only be a way to fail
 * later than necessary.
 */
export function encodeFundingQuoteV1(input: FundingQuoteInputV1): Uint8Array {
  const output = new Uint8Array(CAPABILITY_FUNDING_QUOTE_BYTES_V1);
  const view = new DataView(output.buffer);
  output.set(new TextEncoder().encode(CAPABILITY_FUNDING_QUOTE_MAGIC_V1), 0);
  view.setUint16(CAPABILITY_FUNDING_QUOTE_SCHEMA_OFFSET_V1, CAPABILITY_FUNDING_QUOTE_SCHEMA_VERSION_V1, true);

  let nativeTotal = 0n;
  let realmTotal = 0n;
  for (const compartment of FUNDING_COMPARTMENTS_V1) {
    const funding = input.compartments[compartment.name] ?? NOT_APPLICABLE_V1;
    const classIndex = FUNDING_ASSET_CLASSES_V1.indexOf(funding.assetClass);
    if (classIndex < 0) throw new Error(`${compartment.name} names an unknown funding asset class`);
    if ((funding.amount === 0n) !== (classIndex === 0)) {
      throw new Error(`${compartment.name} states an asset class its amount contradicts`);
    }
    if (compartment.assetPolicy === 'native-lamports-only' && funding.assetClass === 'realm-collateral') {
      throw new Error(`${compartment.name} pays for account existence and admits native lamports only`);
    }
    const offset = CAPABILITY_FUNDING_QUOTE_AMOUNTS_OFFSET_V1 + compartment.offset;
    output[offset + CAPABILITY_FUNDING_ALLOCATION_CLASS_OFFSET_V1] = classIndex;
    view.setBigUint64(offset + CAPABILITY_FUNDING_ALLOCATION_AMOUNT_OFFSET_V1, funding.amount, true);
    if (funding.assetClass === 'native-lamports') nativeTotal += funding.amount;
    if (funding.assetClass === 'realm-collateral') realmTotal += funding.amount;
    if (nativeTotal > MAX_U64 || realmTotal > MAX_U64) throw new Error('a capability funding compartment total overflows u64');
  }
  const amounts = CAPABILITY_FUNDING_QUOTE_AMOUNTS_OFFSET_V1;
  view.setBigUint64(amounts + CAPABILITY_FUNDING_AMOUNTS_NATIVE_TOTAL_OFFSET_V1, nativeTotal, true);
  view.setBigUint64(amounts + CAPABILITY_FUNDING_AMOUNTS_REALM_TOTAL_OFFSET_V1, realmTotal, true);

  if ((realmTotal > 0n) !== (input.realmCollateral !== null)) {
    throw new Error('a Realm collateral binding is present exactly when the Realm total is nonzero');
  }
  output[CAPABILITY_FUNDING_QUOTE_COLLATERAL_KIND_OFFSET_V1] = input.realmCollateral === null ? 0 : 1;
  if (input.realmCollateral !== null) {
    const binding = CAPABILITY_FUNDING_QUOTE_BINDING_OFFSET_V1;
    const fields = [
      [CAPABILITY_FUNDING_BINDING_REALM_ID_OFFSET_V1, input.realmCollateral.realmId, 'Realm identity'],
      [CAPABILITY_FUNDING_BINDING_RELEASE_ID_OFFSET_V1, input.realmCollateral.collateralReleaseId, 'collateral release'],
      [CAPABILITY_FUNDING_BINDING_TOKEN_PROGRAM_OFFSET_V1, input.realmCollateral.tokenProgram, 'token program'],
      [CAPABILITY_FUNDING_BINDING_MINT_OFFSET_V1, input.realmCollateral.mint, 'collateral Mint'],
      [CAPABILITY_FUNDING_BINDING_BENEFICIARY_OFFSET_V1, input.realmCollateral.refundTokenBeneficiary, 'refund beneficiary'],
    ] as const;
    for (const [offset, value, field] of fields) output.set(identity(value, field), binding + offset);
  }
  return output;
}

/** Encode one 528-byte `CapabilityEntryV1`. */
export function encodeCapabilityEntryV1(input: CapabilityEntryInputV1): Uint8Array {
  const output = new Uint8Array(CAPABILITY_ENTRY_BYTES_V1);
  const view = new DataView(output.buffer);
  const identities = [
    [CAPABILITY_ENTRY_KIND_ID_OFFSET_V1, input.kindId, 'capability kind'],
    [CAPABILITY_ENTRY_RELEASE_ID_OFFSET_V1, input.releaseId, 'capability release'],
    [CAPABILITY_ENTRY_CONFIG_ID_OFFSET_V1, input.configId, 'capability config'],
    [CAPABILITY_ENTRY_CAPACITY_PROFILE_ID_OFFSET_V1, input.capacityProfileId, 'capability capacity profile'],
    [CAPABILITY_ENTRY_CHILD_SCHEMA_ID_OFFSET_V1, input.childSchemaId, 'capability child schema'],
    [CAPABILITY_ENTRY_CHILD_DERIVATION_ID_OFFSET_V1, input.childDerivationId, 'capability child derivation'],
  ] as const;
  for (const [offset, value, field] of identities) output.set(identity(value, field), offset);

  output[CAPABILITY_ENTRY_ACTIVATION_POLICY_OFFSET_V1] = input.activation === 'RequiredAtFounding' ? 0 : 1;
  if (input.dependencies.length > MAX_DEPENDENCIES_PER_CAPABILITY_V1) {
    throw new Error(`a capability may declare at most ${MAX_DEPENDENCIES_PER_CAPABILITY_V1} dependencies`);
  }
  output[CAPABILITY_ENTRY_DEPENDENCY_COUNT_OFFSET_V1] = input.dependencies.length;
  input.dependencies.forEach((dependency, position) => {
    if (!Number.isSafeInteger(dependency) || dependency < 0 || dependency > 0xff) throw new Error('a capability dependency index is outside u8');
    if (position > 0 && input.dependencies[position - 1] >= dependency) throw new Error('capability dependencies are not strictly increasing');
    output[CAPABILITY_ENTRY_DEPENDENCIES_OFFSET_V1 + position] = dependency;
  });
  if (typeof input.activationDeadlineSlot !== 'bigint' || input.activationDeadlineSlot < 0n || input.activationDeadlineSlot > MAX_U64) {
    throw new Error('capability activation deadline is outside u64');
  }
  view.setBigUint64(CAPABILITY_ENTRY_ACTIVATION_DEADLINE_OFFSET_V1, input.activationDeadlineSlot, true);
  output.set(encodeFundingQuoteV1(input.quote), CAPABILITY_ENTRY_QUOTE_OFFSET_V1);
  return output;
}

/**
 * Encode a whole manifest, then read it back with the Found path's own decoder.
 *
 * The read-back is the point. Entry ordering, dependency acyclicity, and the
 * activation/prepaid-funding join are all *manifest*-level rules that no single
 * entry can violate on its own, and `validateCoreFoundCapabilityManifestV1` is
 * the browser reader that already enforces every one of them. Encoding and then
 * refusing to return anything that reader would reject means a wizard cannot
 * produce a manifest that its own Found preflight would later refuse.
 */
export function encodeCapabilityManifestV1(entries: ReadonlyArray<CapabilityEntryInputV1>): Uint8Array {
  if (entries.length === 0 || entries.length > MAX_CAPABILITIES_V1) {
    throw new Error(`a manifest carries 1..${MAX_CAPABILITIES_V1} capability entries`);
  }
  const output = new Uint8Array(CAPABILITY_MANIFEST_HEADER_BYTES_V1 + entries.length * CAPABILITY_ENTRY_BYTES_V1);
  const view = new DataView(output.buffer);
  output.set(new TextEncoder().encode(CAPABILITY_MANIFEST_MAGIC_V1), 0);
  view.setUint16(CAPABILITY_MANIFEST_SCHEMA_OFFSET_V1, CAPABILITY_MANIFEST_SCHEMA_VERSION_V1, true);
  view.setUint16(CAPABILITY_MANIFEST_PROFILE_OFFSET_V1, CAPABILITY_MANIFEST_ARTIFACT_PROFILE_V1, true);
  view.setUint16(CAPABILITY_MANIFEST_COUNT_OFFSET_V1, entries.length, true);
  entries.forEach((entry, index) => {
    output.set(encodeCapabilityEntryV1(entry), CAPABILITY_MANIFEST_HEADER_BYTES_V1 + index * CAPABILITY_ENTRY_BYTES_V1);
  });
  validateCoreFoundCapabilityManifestV1(output);
  return output;
}

/**
 * Sort entries into the canonical order a manifest requires.
 *
 * Entries must be strictly increasing by kind identity, which makes the kind
 * unique per manifest and the index a stable coordinate. A wizard collects them
 * in whatever order an operator adds them, so the sort belongs here — but it
 * refuses a duplicate kind rather than silently keeping one, because two
 * capabilities of the same kind is a statement about the Market that this
 * schema cannot represent.
 */
export function canonicalCapabilityOrderV1(entries: ReadonlyArray<CapabilityEntryInputV1>): ReadonlyArray<CapabilityEntryInputV1> {
  const sorted = [...entries].sort((left, right) => (left.kindId < right.kindId ? -1 : left.kindId > right.kindId ? 1 : 0));
  for (let index = 1; index < sorted.length; index += 1) {
    if (sorted[index - 1].kindId === sorted[index].kindId) throw new Error('two capability entries declare the same kind identity');
  }
  return Object.freeze(sorted);
}

export type FundingTotalsV1 = Readonly<{
  perCompartment: ReadonlyArray<Readonly<{ name: FundingCompartmentNameV1; assetClass: FundingAssetClassV1; amount: bigint }>>;
  nativeLamports: bigint;
  realmCollateral: bigint;
}>;

/**
 * Sum one manifest's quotes for display, keeping the two assets apart.
 *
 * Returned as two fields and never as a sum. The whole reason the wire carries
 * separate checked totals is that a lamport and a collateral atom are not
 * commensurable, and a summary that added them would be the first place in the
 * system to claim otherwise.
 */
export function summarizeManifestFundingV1(entries: ReadonlyArray<CapabilityEntryInputV1>): FundingTotalsV1 {
  const perCompartment = FUNDING_COMPARTMENTS_V1.map((compartment) => {
    const fundings = entries.map((entry) => entry.quote.compartments[compartment.name] ?? NOT_APPLICABLE_V1);
    const amount = fundings.reduce((total, funding) => total + funding.amount, 0n);
    const classes = new Set(fundings.filter((funding) => funding.amount > 0n).map((funding) => funding.assetClass));
    if (classes.size > 1) throw new Error(`${compartment.name} is quoted in two different asset classes across entries`);
    return Object.freeze({
      name: compartment.name,
      assetClass: (classes.values().next().value ?? 'not-applicable') as FundingAssetClassV1,
      amount,
    });
  });
  return Object.freeze({
    perCompartment: Object.freeze(perCompartment),
    nativeLamports: perCompartment.filter((entry) => entry.assetClass === 'native-lamports').reduce((total, entry) => total + entry.amount, 0n),
    realmCollateral: perCompartment.filter((entry) => entry.assetClass === 'realm-collateral').reduce((total, entry) => total + entry.amount, 0n),
  });
}
