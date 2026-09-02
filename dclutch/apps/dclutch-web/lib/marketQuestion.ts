import { fromHex, i64, isZero, sha256 } from './bytes';
import {
  decodeCoreFoundProductGraphV2,
  decodeResultDomainV2,
  deriveCoreFoundRecordsV2,
  type ResultDomainV2,
} from './coreFound';
import { formatTicksV1 } from './founding/rangeProtection';
import {
  PORTFOLIO_SCHEMA_ID_V2,
  PRODUCT_RECORD_SCHEMA_ID_V2,
  RESULT_DOMAIN_SCHEMA_ID_V2,
  SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
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

function record(accounts: ReadonlyMap<string, RpcAccount | null>, address: string, registry: string, field: string): Uint8Array {
  const account = accounts.get(address) ?? null;
  if (account === null) throw new Error(`${field} does not exist at ${address}`);
  if (account.owner !== registry || account.executable) throw new Error(`${field} is not a nonexecutable Registry-owned raw record`);
  return account.data;
}

/**
 * Read one market's partition and window from the chain.
 *
 * Two round trips. The first is `deriveCoreFoundRecordsV2`, which is the
 * semantic owner of "authenticate the two parents, then derive the children
 * they name" — it re-hashes both parents against their own schema/content
 * PDAs and refuses a SourceMaterialV3 that is about a different Product before
 * any child address exists. The second reads the three children and proves
 * each is the record its parent named, by hashing it and re-deriving the same
 * PDA.
 *
 * The Product root is re-read in the second round on purpose:
 * `decodeCoreFoundProductGraphV2` is the check that the domain and portfolio
 * here are the ones this product SELECTS, and it needs the root's bytes to
 * say so. A cheaper version that trusted the first round's derivation would be
 * trusting the same client that produced the addresses.
 */
export async function inspectMarketQuestionV1(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts'>,
  request: MarketQuestionRequestV1,
): Promise<MarketQuestionV1> {
  const registry = request.registryProgramId;
  const productDigest = fromHex(request.productRecordId, 'Market product record identity');
  const sourceDigest = fromHex(request.resolutionPolicyId, 'Market resolution policy identity');
  if (isZero(productDigest) || isZero(sourceDigest)) throw new Error('this Market names an all-zero record identity');
  const productRecord = deriveFinalizedRecordAddressesV1(registry, PRODUCT_RECORD_SCHEMA_ID_V2, productDigest).record;
  const sourceMaterialRecord = deriveFinalizedRecordAddressesV1(registry, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, sourceDigest).record;

  const derived = await deriveCoreFoundRecordsV2(client, { registryProgram: registry, productRecord, sourceMaterialRecord });

  const addresses = [productRecord, derived.resultDomainRecord, derived.portfolioRecord, derived.windowSpecRecord];
  const observation = await client.multipleAccounts(addresses, derived.observedSlot);
  const accounts = new Map(observation.accounts.map((entry) => [entry.address, entry.account]));

  const product = record(accounts, productRecord, registry, 'Product record');
  const domain = record(accounts, derived.resultDomainRecord, registry, 'result domain record');
  const portfolio = record(accounts, derived.portfolioRecord, registry, 'portfolio record');
  const domainDigest = await sha256(domain);
  const portfolioDigest = await sha256(portfolio);

  // The full join, by the founding's own decoder: the Product root selects
  // exactly these two children, the domain carries the product identity, the
  // portfolio joins on both, its coefficients are gcd-normalized, and the
  // outcome width is regions + 1.
  const graph = decodeCoreFoundProductGraphV2(product, domain, portfolio, domainDigest, portfolioDigest);
  const partition: ResultDomainV2 = decodeResultDomainV2(domain);

  // The window is decoded separately and is allowed to be absent. Every
  // market has a partition — a Found refuses without one — but a window
  // record is published beside the founding rather than consumed by it, so a
  // market whose window record was never published is a real state, and the
  // page says so rather than dropping the whole derivation.
  let window: MarketWindowV1 | null = null;
  let windowRefusal: string | null = null;
  try {
    const bytes = record(accounts, derived.windowSpecRecord, registry, 'window record');
    if (deriveFinalizedRecordAddressesV1(registry, WINDOW_SPEC_SCHEMA_ID_V1, await sha256(bytes)).record !== derived.windowSpecRecord) {
      throw new Error('the window record is not the schema/content-derived Registry raw PDA');
    }
    window = decodeWindowSpecV1(bytes);
  } catch (error) {
    windowRefusal = error instanceof Error ? error.message : String(error);
  }

  if (deriveFinalizedRecordAddressesV1(registry, RESULT_DOMAIN_SCHEMA_ID_V2, domainDigest).record !== derived.resultDomainRecord
    || deriveFinalizedRecordAddressesV1(registry, PORTFOLIO_SCHEMA_ID_V2, portfolioDigest).record !== derived.portfolioRecord) {
    throw new Error('a child record is not the schema/content-derived Registry raw PDA');
  }

  return Object.freeze({
    address: request.address,
    observedSlot: observation.slot,
    productRecord,
    sourceMaterialRecord,
    resultDomainRecord: derived.resultDomainRecord,
    portfolioRecord: derived.portfolioRecord,
    windowSpecRecord: derived.windowSpecRecord,
    cutDenominator: partition.denominator,
    cuts: partition.cuts,
    regionCount: partition.regionCount,
    outcomeCount: graph.outcomeCount,
    window,
    windowRefusal,
  });
}
