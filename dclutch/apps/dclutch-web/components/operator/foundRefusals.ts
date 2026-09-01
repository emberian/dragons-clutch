/**
 * Every refusal `/found` can show, routed to the field that owns it.
 *
 * OPERATOR_FORMS_V1 §6, and the console that needed it most. `/found` has
 * sixteen fields and ONE `aria-live` line, so today every refusal — from a
 * mistyped endpoint to a cyclic capability graph — arrives in the same slot
 * with the same prefix. `lib/tradeFlowRefusals.ts` states why that is not a
 * presentation quibble: it "tells a reader that something is wrong somewhere
 * behind them."
 *
 * The worst instance is structural. `prepareCoreFoundV2` validates the ten
 * record addresses as an array and names them by POSITION —
 * `finalized raw record 4 must be canonical base58 text`
 * (`lib/coreFound.ts:650`) — while the screen labels that same field
 * `Linked basis raw`. The mapping between the two exists only in the reader's
 * head. This table is that mapping, written down.
 *
 * The design is `assignRefusalV1`'s, deliberately: a data table matched with
 * `includes`, most specific first, so the SDK's conditions are not touched to
 * carry a code. A diff in `coreFound.ts` would mean the extraction went wrong.
 *
 * The one rule that keeps this honest: **a refusal whose owner is ambiguous is
 * not guessed.** It returns `routed: false` and renders at form level with a
 * fallback remedy, exactly as the trade flow does. Attributing a refusal to
 * the wrong field is worse than attributing it to none.
 */

/** The record addresses, in the order `prepareCoreFoundV2` reads them. */
export const FOUND_RAW_RECORD_ORDER_V1: ReadonlyArray<Readonly<{ field: string; label: string }>> = Object.freeze([
  Object.freeze({ field: 'realmRecord', label: 'Realm raw record' }),
  Object.freeze({ field: 'productRecord', label: 'Product Runtime V2 raw' }),
  Object.freeze({ field: 'resultDomainRecord', label: 'Result domain raw' }),
  Object.freeze({ field: 'portfolioRecord', label: 'Portfolio raw' }),
  Object.freeze({ field: 'linkedBasisRecord', label: 'Linked basis raw' }),
  Object.freeze({ field: 'sourceMaterialRecord', label: 'SourceMaterialV3 raw' }),
  Object.freeze({ field: 'sourceSpecRecord', label: 'Source spec raw' }),
  Object.freeze({ field: 'capacityProfileRecord', label: 'Source capacity profile raw' }),
  Object.freeze({ field: 'manipulationFloorRecord', label: 'Manipulation floor raw' }),
  Object.freeze({ field: 'capabilityManifestRecord', label: 'Capability manifest raw' }),
]);

/** One refusal, routed to its owner and given a remedy. */
export type FoundRefusalV1 = Readonly<{
  /** The state key of the field that can act on this, or null for form level. */
  field: string | null;
  /** What the reader can do, in one imperative sentence. Rendered first. */
  remedy: string;
  /** The refusal, exactly as it was produced. Rendered whole, in one element. */
  detail: string;
  /** True when a fragment matched. False keeps it at form level, unattributed. */
  routed: boolean;
}>;

type OwnerV1 = Readonly<{ fragment: string; field: string | null; remedy: string }>;

const POSITIONAL_OWNERS_V1: ReadonlyArray<OwnerV1> = Object.freeze(
  FOUND_RAW_RECORD_ORDER_V1.map((record, index) => Object.freeze({
    fragment: `finalized raw record ${index}`,
    field: record.field,
    remedy: `Check the address in ${record.label}.`,
  })),
);

const OWNERS_V1: ReadonlyArray<OwnerV1> = Object.freeze([
  // ---- The positional refusals, which name a record by index and nothing else.
  ...POSITIONAL_OWNERS_V1,

  // ---- The chain this founds against. ------------------------------------
  Object.freeze({
    fragment: 'RPC endpoint must use http or https',
    field: 'endpoint',
    remedy: 'Enter an http or https endpoint.',
  }),
  Object.freeze({
    // The URL constructor's own words, reached through an unguarded `new URL`
    // at `lib/rpc.ts`. Routing it is the only way a reader learns which of the
    // sixteen fields it came from.
    fragment: 'Invalid URL',
    field: 'endpoint',
    remedy: 'Enter the endpoint as a full URL, scheme included.',
  }),

  // ---- The generation. ----------------------------------------------------
  Object.freeze({ fragment: 'generation must be a canonical unsigned integer', field: 'generation', remedy: 'Enter the generation as digits only.' }),
  Object.freeze({ fragment: 'generation exceeds u64', field: 'generation', remedy: 'Enter a generation inside u64.' }),
  Object.freeze({ fragment: 'Market generation is outside lifecycle u64', field: 'generation', remedy: 'Enter a generation of 1 or more.' }),

  // ---- Who pays, and who is refunded. -------------------------------------
  Object.freeze({ fragment: 'payer is not a System-owned data-free wallet', field: 'payer', remedy: 'Use a plain wallet address as the payer.' }),
  Object.freeze({ fragment: 'payer must be canonical base58 text', field: 'payer', remedy: 'Paste the payer address as base58.' }),
  Object.freeze({ fragment: 'refund wallet is not a System-owned data-free wallet', field: 'refundWallet', remedy: 'Use a plain wallet address as the refund wallet.' }),
  Object.freeze({ fragment: 'refund wallet must be canonical base58 text', field: 'refundWallet', remedy: 'Paste the refund wallet address as base58.' }),

  // ---- The deployment. ----------------------------------------------------
  Object.freeze({ fragment: 'Registry program must be canonical base58 text', field: 'registryProgram', remedy: 'Paste the Registry program address as base58.' }),
  Object.freeze({ fragment: 'activation cache is not the release-derived Registry PDA', field: 'activationCache', remedy: 'Use the activation cache this release derives.' }),
  Object.freeze({ fragment: 'activation cache has the wrong Registry owner or executable flag', field: 'activationCache', remedy: 'Use a Registry-owned activation cache.' }),
  Object.freeze({ fragment: 'activation cache has the wrong exact width, magic, schema, or profile', field: 'activationCache', remedy: 'Use the activation cache this release published.' }),
  Object.freeze({ fragment: 'activation cache must be canonical base58 text', field: 'activationCache', remedy: 'Paste the activation cache address as base58.' }),

  // ---- Records whose own decode named them. -------------------------------
  Object.freeze({ fragment: 'Realm is not the schema/content-derived Registry raw PDA', field: 'realmRecord', remedy: 'Use the Realm record this schema and digest derive.' }),
  Object.freeze({ fragment: 'Realm record has the wrong exact ABI', field: 'realmRecord', remedy: 'Point Realm raw record at a Realm record.' }),
  Object.freeze({ fragment: 'Product Runtime V2 root has the wrong exact ABI', field: 'productRecord', remedy: 'Point Product Runtime V2 raw at a Product record.' }),
  Object.freeze({ fragment: 'Product is not the schema/content-derived Registry raw PDA', field: 'productRecord', remedy: 'Use the Product record this schema and digest derive.' }),
  Object.freeze({ fragment: 'Product result domain has the wrong exact ABI', field: 'resultDomainRecord', remedy: 'Point Result domain raw at a result-domain record.' }),
  Object.freeze({ fragment: 'Product portfolio has the wrong exact ABI', field: 'portfolioRecord', remedy: 'Point Portfolio raw at a portfolio record.' }),
  Object.freeze({ fragment: 'linked basis is not the schema/content-derived Registry raw PDA', field: 'linkedBasisRecord', remedy: 'Use the linked basis record this schema and digest derive.' }),
  Object.freeze({ fragment: 'SourceMaterialV3 selects a different Product record digest', field: 'sourceMaterialRecord', remedy: 'Use the SourceMaterialV3 record that selects this Product.' }),
  Object.freeze({ fragment: 'SourceMaterialV3 has the wrong exact ABI', field: 'sourceMaterialRecord', remedy: 'Point SourceMaterialV3 raw at a SourceMaterialV3 record.' }),
  Object.freeze({ fragment: 'capability dependency graph is cyclic', field: 'capabilityManifestRecord', remedy: 'Use a capability manifest whose dependencies terminate.' }),
  Object.freeze({ fragment: 'capability manifest has the wrong exact header', field: 'capabilityManifestRecord', remedy: 'Point Capability manifest raw at a capability manifest.' }),
]);

/**
 * The refusals this table deliberately does NOT route, and why.
 *
 * Each names a JOIN between records rather than one record, so the field that
 * can fix it depends on which side is wrong — and the console cannot know
 * which. They render at form level with the fallback remedy, which is the
 * honest outcome: the reader is told a relationship failed and which
 * relationship, rather than being pointed at a field that may be correct.
 */
export const FOUND_UNROUTED_BY_DESIGN_V1: ReadonlyArray<string> = Object.freeze([
  'Product root does not select the supplied domain and portfolio',
  'SourceMaterialV3 graph identities differ from the authenticated records',
  'Found authority inputs alias named roles',
]);

const FALLBACK_REMEDY_V1 = 'This construction refused. Its own words are below.';

export function assignFoundRefusalV1(detail: string): FoundRefusalV1 {
  for (const owner of OWNERS_V1) {
    if (detail.includes(owner.fragment)) {
      return Object.freeze({ field: owner.field, remedy: owner.remedy, detail, routed: true });
    }
  }
  return Object.freeze({ field: null, remedy: FALLBACK_REMEDY_V1, detail, routed: false });
}

/** Every fragment this table routes, for the test that keeps it honest. */
export function routedFoundFragmentsV1(): ReadonlyArray<string> {
  return Object.freeze(OWNERS_V1.map((owner) => owner.fragment));
}
