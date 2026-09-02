import { fromHex, i64, isZero, slice } from './bytes';
import {
  acquireFinalizedAccountsInChunksV1,
  authenticateFinalizedRawRecordV2,
  decodeCoreFoundProductGraphV2,
  decodeResultDomainV2,
  validateCoreFoundSourceMaterialV3,
  type ResultDomainV2,
} from './coreFound';
import { formatTicksV1 } from './founding/rangeProtection';
import {
  PORTFOLIO_SCHEMA_ID_V2,
  PRODUCT_RECORD_DOMAIN_DIGEST_OFFSET_V2,
  PRODUCT_RECORD_PORTFOLIO_DIGEST_OFFSET_V2,
  PRODUCT_RECORD_SCHEMA_ID_V2,
  RESULT_DOMAIN_SCHEMA_ID_V2,
  SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
  SOURCE_MATERIAL_WINDOW_SPEC_OFFSET_V3,
  WINDOW_SPEC_BYTES_V1,
  WINDOW_SPEC_END_UNIX_SECONDS_OFFSET_V1,
  WINDOW_SPEC_MAGIC,
  WINDOW_SPEC_SCHEMA_ID_V1,
  WINDOW_SPEC_START_UNIX_SECONDS_OFFSET_V1,
} from './generated/coreFound';
import { deriveFinalizedRecordAddressesV1 } from './releaseRegistry';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

/**
 * What a market ASKS, derived from the market's own records.
 *
 * A Core Market stores no words, so the site has always taken its question,
 * its outcome names and its settlement time from one hand-written editorial
 * file keyed by market address (`lib/marketRegistry.ts`). That file has now
 * lagged every redeploy this project has done: on 2026-09-02 it named six
 * markets on a Core program that had been closed the day before, and named
 * the one open market not at all — so the live market rendered as
 * `Unnamed · EQnY…mGs1`, "Outcomes 4", and "No settlement time is published",
 * on a page whose own chain reads already held every fact it was refusing to
 * state.
 *
 * They were never editorial facts. The outcome partition is a
 * `ResultDomainV2` record — an exact denominator and an ordered list of cuts —
 * and the settlement window is a `WindowSpecV1` record, both content-addressed
 * children of records the Market names in its own identity seeds. This module
 * reads them, and the editorial registry becomes an OVERRIDE that may add a
 * coordinate's name and a story rather than the sole author of what a market
 * is.
 *
 * WHAT THE CHAIN CANNOT SAY, said plainly. The coordinate itself — "SOL/USD" —
 * is a content identity in the product and source records, not text: nothing
 * on chain spells it. So the labels below are shaped by the chain (how many
 * cells, where the boundaries fall, which cell is the source-failure outcome)
 * and NAMED by the registry only where a name is genuinely absent from the
 * wire. A market with no registry row still gets its real boundaries, its real
 * cell count and its real settlement time; it is missing exactly one thing,
 * the coordinate's common name, and it says so instead of inventing one.
 *
 * Nothing here can move a number. Every quantity is decoded by
 * `decodeResultDomainV2` and `decodeCoreFoundProductGraphV2`, which are the
 * same decoders the founding preflight runs, and every record is authenticated
 * as its own schema/content PDA before a byte of it is read.
 */

/** How a market's coordinate is named, where the wire has no name for it. */
export type CoordinateNamingV1 = Readonly<{
  /** What the cuts measure, e.g. `SOL/USD`. Editorial; never chain-read. */
  label: string;
  /** A unit written before each boundary, e.g. `$`. Editorial. */
  unitPrefix: string | null;
}>;

/** The market's settlement window, in whole seconds since the Unix epoch. */
export type MarketWindowV1 = Readonly<{
  startUnixSeconds: bigint;
  endUnixSeconds: bigint;
}>;

/** One market's own outcome partition and window, read from its records. */
export type MarketQuestionV1 = Readonly<{
  address: string;
  /** The finalized floor the child records were read at. */
  observedSlot: string;
  productRecord: string;
  sourceMaterialRecord: string;
  resultDomainRecord: string;
  portfolioRecord: string;
  windowSpecRecord: string;
  /** Ticks per whole unit of the coordinate. */
  cutDenominator: bigint;
  /** The interior boundaries, strictly increasing, in ticks. */
  cuts: ReadonlyArray<bigint>;
  /** Ordinary cells: always `cuts.length + 1`. */
  regionCount: number;
  /** Cells including the explicit source-failure outcome, always the last. */
  outcomeCount: number;
  /** The settlement window, or null with `windowRefusal` saying why not. */
  window: MarketWindowV1 | null;
  windowRefusal: string | null;
}>;

/**
 * The boundary text for one cut, as a reader reads it.
 *
 * Exact by construction: `formatTicksV1` divides by the denominator in
 * integers, so a cut of 9,800 over 100 is `98` and never `97.99999999999999`.
 */
export function formatBoundaryV1(ticks: bigint, denominator: bigint, naming: CoordinateNamingV1 | null): string {
  return `${naming?.unitPrefix ?? ''}${formatTicksV1(ticks, denominator)}`;
}

/**
 * Index-ordered outcome labels, derived from the partition.
 *
 * `regionCount` ordinary cells then the explicit source-failure outcome, which
 * is what `decodeCoreFoundProductGraphV2` proves the width is: outcomes are
 * exactly regions plus one, and the extra one is the failure cell.
 */
export function derivedOutcomeLabelsV1(
  partition: Pick<MarketQuestionV1, 'cuts' | 'cutDenominator' | 'regionCount' | 'outcomeCount'>,
  naming: CoordinateNamingV1 | null,
): ReadonlyArray<string> {
  const at = (index: number): string => formatBoundaryV1(partition.cuts[index]!, partition.cutDenominator, naming);
  const labels: string[] = [];
  for (let index = 0; index < partition.regionCount; index += 1) {
    if (partition.cuts.length === 0) labels.push('Any value the source reports');
    else if (index === 0) labels.push(`Below ${at(0)}`);
    else if (index === partition.regionCount - 1) labels.push(`${at(index - 1)} and above`);
    else labels.push(`${at(index - 1)} – ${at(index)}`);
  }
  labels.push('The source failed to report');
  while (labels.length < partition.outcomeCount) labels.push(`Claim ${labels.length}`);
  return Object.freeze(labels.slice(0, partition.outcomeCount));
}

/** The question sentence, derived. Names the coordinate only if named. */
export function derivedQuestionV1(
  partition: Pick<MarketQuestionV1, 'cuts' | 'cutDenominator' | 'regionCount' | 'outcomeCount'>,
  naming: CoordinateNamingV1 | null,
): string {
  const subject = naming === null ? 'the value this market measures' : naming.label;
  if (partition.cuts.length === 0) {
    return `Did the source report a value for ${subject} at all?`;
  }
  const cells = derivedOutcomeLabelsV1(partition, naming)
    .slice(0, partition.regionCount)
    .map((label) => label.toLowerCase());
  const listed = cells.length === 1 ? cells[0]! : `${cells.slice(0, -1).join(', ')}, or ${cells[cells.length - 1]!}`;
  return `Where does ${subject} finish inside this market's window — ${listed}? If the source cannot answer, it settles on the source-failure outcome.`;
}

/** A display title, derived. Never an address, and never an invented name. */
export function derivedTitleV1(
  partition: Pick<MarketQuestionV1, 'cuts' | 'cutDenominator' | 'regionCount' | 'outcomeCount'>,
  naming: CoordinateNamingV1 | null,
): string {
  const subject = naming === null ? 'An unnamed observable' : naming.label;
  if (partition.cuts.length === 0) return `${subject} — did the source report`;
  if (partition.cuts.length === 1) {
    return `${subject} — above or below ${formatBoundaryV1(partition.cuts[0]!, partition.cutDenominator, naming)}`;
  }
  const first = formatBoundaryV1(partition.cuts[0]!, partition.cutDenominator, naming);
  const last = formatBoundaryV1(partition.cuts[partition.cuts.length - 1]!, partition.cutDenominator, naming);
  return `${subject} — ${partition.regionCount} ways past ${first} and ${last}`;
}

/**
 * A settlement time, written the one way that cannot be misread.
 *
 * UTC and explicit. A market's window is a fact about the world, and rendering
 * it in the reader's local zone with no zone printed is how two readers of the
 * same page disagree about when a market closed.
 */
export function formatWindowInstantV1(unixSeconds: bigint): string {
  const milliseconds = Number(unixSeconds) * 1000;
  if (!Number.isFinite(milliseconds)) throw new Error('window instant is outside the representable range');
  const moment = new Date(milliseconds);
  const pad = (value: number, width = 2): string => String(value).padStart(width, '0');
  return `${moment.getUTCFullYear()}-${pad(moment.getUTCMonth() + 1)}-${pad(moment.getUTCDate())} `
    + `${pad(moment.getUTCHours())}:${pad(moment.getUTCMinutes())} UTC`;
}

/** Decode one exact `WindowSpecV1` preimage: magic, width, and both bounds. */
export function decodeWindowSpecV1(bytes: Uint8Array): MarketWindowV1 {
  // The magic is compared against the emitted constant itself rather than
  // sliced out at a literal offset: every dClutch record carries its magic
  // first, and writing that `0` here would be this browser stating a wire
  // coordinate in its own words -- which `abi-coverage` counts, and caught.
  if (bytes.length !== WINDOW_SPEC_BYTES_V1
    || WINDOW_SPEC_MAGIC.some((byte, index) => bytes[index] !== byte)) {
    throw new Error('window spec has the wrong exact ABI');
  }
  const startUnixSeconds = i64(bytes, WINDOW_SPEC_START_UNIX_SECONDS_OFFSET_V1);
  const endUnixSeconds = i64(bytes, WINDOW_SPEC_END_UNIX_SECONDS_OFFSET_V1);
  // Both window kinds are closed intervals and both require start <= end, so a
  // record that fails it is not a narrower window; it is not a window.
  if (startUnixSeconds > endUnixSeconds) throw new Error('window spec bounds are not ordered');
  return Object.freeze({ startUnixSeconds, endUnixSeconds });
}

export type MarketQuestionRequestV1 = Readonly<{
  registryProgramId: string;
  address: string;
  /** The Market's `productRecordId` identity seed, hex. */
  productRecordId: string;
  /** The Market's `resolutionPolicyId` identity seed, hex. */
  resolutionPolicyId: string;
}>;

/**
 * What one market's read produced: its question, or the reason there is none.
 *
 * A batch answers per market. One market whose product record was never
 * published must not cost the other nineteen their questions, and a batch that
 * threw would do exactly that.
 */
export type MarketQuestionOutcomeV1 =
  | Readonly<{ status: 'derived'; question: MarketQuestionV1 }>
  | Readonly<{ status: 'refused'; address: string; reason: string }>;

/** One error, as a sentence, without letting a non-Error reach a reader raw. */
function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * Read many markets' partitions and windows in two round trips, not two each.
 *
 * The market LIST needs this. A page of twenty cards deriving one at a time is
 * forty sequential round trips, which is why the list carried registry titles
 * only and an unregistered market listed by its address while the page it
 * linked to knew better. Two observations serve any number of markets: the
 * parents in the first, every child in the second, both through the 32-key
 * chunker so a long list does not exceed the RPC bound.
 *
 * The authentication is unchanged and is per record, not per batch: every
 * parent is `authenticateFinalizedRawRecordV2` -- owner, executable bit, and
 * the schema/content PDA it must hash to -- every SourceMaterialV3 must be
 * about its own Product, and every child must hash to the address its parent
 * named. Reading fifty records in one call buys latency and buys nothing else.
 */
export async function inspectMarketQuestionsV1(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts' | 'multipleAccountDataSlices'>,
  request: Readonly<{ registryProgramId: string; markets: ReadonlyArray<Omit<MarketQuestionRequestV1, 'registryProgramId'>> }>,
): Promise<ReadonlyArray<MarketQuestionOutcomeV1>> {
  const registry = request.registryProgramId;
  if (request.markets.length === 0) return Object.freeze([]);

  type Pending = {
    address: string;
    productRecord: string;
    sourceMaterialRecord: string;
    productBytes: Uint8Array | null;
    children: Readonly<{ domain: string; portfolio: string; window: string }> | null;
    reason: string | null;
  };
  const pending: Pending[] = request.markets.map((market) => {
    try {
      const productDigest = fromHex(market.productRecordId, 'Market product record identity');
      const sourceDigest = fromHex(market.resolutionPolicyId, 'Market resolution policy identity');
      if (isZero(productDigest) || isZero(sourceDigest)) throw new Error('this Market names an all-zero record identity');
      return {
        address: market.address,
        productRecord: deriveFinalizedRecordAddressesV1(registry, PRODUCT_RECORD_SCHEMA_ID_V2, productDigest).record,
        sourceMaterialRecord: deriveFinalizedRecordAddressesV1(registry, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, sourceDigest).record,
        productBytes: null, children: null, reason: null,
      };
    } catch (error) {
      return { address: market.address, productRecord: '', sourceMaterialRecord: '', productBytes: null, children: null, reason: message(error) };
    }
  });

  const floor = await client.finalizedSlot();
  const parentAddresses = [...new Set(pending.filter((entry) => entry.reason === null).flatMap((entry) => [entry.productRecord, entry.sourceMaterialRecord]))];
  if (parentAddresses.length > 0) {
    const observation = await acquireFinalizedAccountsInChunksV1(client, parentAddresses, floor);
    const accounts = new Map(observation.accounts.map((entry) => [entry.address, entry.account]));
    for (const entry of pending) {
      if (entry.reason !== null) continue;
      try {
        if (entry.productRecord === entry.sourceMaterialRecord) throw new Error('Product and SourceMaterialV3 cannot be the same record');
        const product = await authenticateFinalizedRawRecordV2(accounts.get(entry.productRecord) ?? null, entry.productRecord, registry, PRODUCT_RECORD_SCHEMA_ID_V2, 'Product record');
        const source = await authenticateFinalizedRawRecordV2(accounts.get(entry.sourceMaterialRecord) ?? null, entry.sourceMaterialRecord, registry, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, 'Source material record');
        validateCoreFoundSourceMaterialV3(source.bytes, product.digest);
        const at = (bytes: Uint8Array, offset: number, field: string): Uint8Array => {
          const digest = slice(bytes, offset, 32);
          if (isZero(digest)) throw new Error(`${field} is the all-zero identity`);
          return digest;
        };
        const derive = (schema: Uint8Array, digest: Uint8Array): string => deriveFinalizedRecordAddressesV1(registry, schema, digest).record;
        entry.productBytes = product.bytes;
        entry.children = Object.freeze({
          domain: derive(RESULT_DOMAIN_SCHEMA_ID_V2, at(product.bytes, PRODUCT_RECORD_DOMAIN_DIGEST_OFFSET_V2, 'result domain digest')),
          portfolio: derive(PORTFOLIO_SCHEMA_ID_V2, at(product.bytes, PRODUCT_RECORD_PORTFOLIO_DIGEST_OFFSET_V2, 'portfolio digest')),
          window: derive(WINDOW_SPEC_SCHEMA_ID_V1, at(source.bytes, SOURCE_MATERIAL_WINDOW_SPEC_OFFSET_V3, 'window spec digest')),
        });
      } catch (error) {
        entry.reason = message(error);
      }
    }
  }

  const childAddresses = [...new Set(pending.flatMap((entry) => entry.children === null ? [] : [entry.children.domain, entry.children.portfolio, entry.children.window]))];
  const childAccounts = childAddresses.length === 0
    ? new Map<string, RpcAccount | null>()
    : new Map((await acquireFinalizedAccountsInChunksV1(client, childAddresses, floor)).accounts.map((entry) => [entry.address, entry.account]));

  const results: MarketQuestionOutcomeV1[] = [];
  for (const entry of pending) {
    if (entry.reason !== null || entry.children === null || entry.productBytes === null) {
      results.push(Object.freeze({ status: 'refused' as const, address: entry.address, reason: entry.reason ?? 'the market’s records were not read' }));
      continue;
    }
    try {
      const domain = (await authenticateFinalizedRawRecordV2(childAccounts.get(entry.children.domain) ?? null, entry.children.domain, registry, RESULT_DOMAIN_SCHEMA_ID_V2, 'result domain record'));
      const portfolio = (await authenticateFinalizedRawRecordV2(childAccounts.get(entry.children.portfolio) ?? null, entry.children.portfolio, registry, PORTFOLIO_SCHEMA_ID_V2, 'portfolio record'));
      // The full join, by the founding's own decoder: the Product root selects
      // exactly these two children, the domain carries the product identity,
      // the portfolio joins on both, its coefficients are gcd-normalized, and
      // the outcome width is regions + 1.
      const graph = decodeCoreFoundProductGraphV2(entry.productBytes, domain.bytes, portfolio.bytes, domain.digest, portfolio.digest);
      const partition: ResultDomainV2 = decodeResultDomainV2(domain.bytes);

      // The window is allowed to be absent on its own. Every market has a
      // partition -- a Found refuses without one -- but a market whose window
      // record was never published is a real state, and losing the whole
      // question over it would be the wrong trade.
      let window: MarketWindowV1 | null = null;
      let windowRefusal: string | null = null;
      try {
        const record = await authenticateFinalizedRawRecordV2(childAccounts.get(entry.children.window) ?? null, entry.children.window, registry, WINDOW_SPEC_SCHEMA_ID_V1, 'window record');
        window = decodeWindowSpecV1(record.bytes);
      } catch (error) {
        windowRefusal = message(error);
      }

      results.push(Object.freeze({
        status: 'derived' as const,
        question: Object.freeze({
          address: entry.address,
          observedSlot: floor,
          productRecord: entry.productRecord,
          sourceMaterialRecord: entry.sourceMaterialRecord,
          resultDomainRecord: entry.children.domain,
          portfolioRecord: entry.children.portfolio,
          windowSpecRecord: entry.children.window,
          cutDenominator: partition.denominator,
          cuts: partition.cuts,
          regionCount: partition.regionCount,
          outcomeCount: graph.outcomeCount,
          window,
          windowRefusal,
        }),
      }));
    } catch (error) {
      results.push(Object.freeze({ status: 'refused' as const, address: entry.address, reason: message(error) }));
    }
  }
  return Object.freeze(results);
}

/**
 * Read one market's partition and window.
 *
 * The batch with one element, so the market page and the market list cannot
 * disagree about what a market asks: one reader, one authentication path, one
 * set of refusal words. It throws where the batch reports, because a detail
 * page asking about exactly one market wants the reason, not a list of one.
 */
export async function inspectMarketQuestionV1(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts' | 'multipleAccountDataSlices'>,
  request: MarketQuestionRequestV1,
): Promise<MarketQuestionV1> {
  const [outcome] = await inspectMarketQuestionsV1(client, {
    registryProgramId: request.registryProgramId,
    markets: [Object.freeze({
      address: request.address,
      productRecordId: request.productRecordId,
      resolutionPolicyId: request.resolutionPolicyId,
    })],
  });
  if (outcome === undefined) throw new Error('the market question read returned no answer for the market it was asked about');
  if (outcome.status === 'refused') throw new Error(outcome.reason);
  return outcome.question;
}
