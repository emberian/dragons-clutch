import { createHash } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const [fixturePath, outputDirectory, completionPath, inspectionPath] = process.argv.slice(2);
if ([fixturePath, outputDirectory, completionPath, inspectionPath].some((value) => value === undefined || !value.startsWith('/'))) {
  throw new Error('verifier requires four absolute paths');
}

function bytes(path) { return readFileSync(path); }
function digest(value) { return createHash('sha256').update(value).digest('hex'); }
function document(path, noun) {
  const value = JSON.parse(bytes(path).toString('utf8'));
  if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${noun} is not one object`);
  return value;
}
function exactKeys(value, expected, noun) {
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (actual.length !== wanted.length || actual.some((key, index) => key !== wanted[index])) throw new Error(`${noun} has missing or unknown fields`);
}

const expectedOutputFiles = ['portfolio.bin', 'price-gate.bin', 'product-basis.bin', 'product.bin', 'report.json', 'result-domain.bin'];
const actualOutputFiles = readdirSync(outputDirectory).sort();
if (actualOutputFiles.length !== expectedOutputFiles.length || actualOutputFiles.some((file, index) => file !== expectedOutputFiles[index])) {
  throw new Error('compiler output directory does not contain exactly the five records and report');
}

const completion = document(completionPath, 'compiler completion');
exactKeys(completion, ['schema', 'output_dir', 'report', 'report_sha256'], 'compiler completion');
const reportPath = join(outputDirectory, 'report.json');
const reportBytes = bytes(reportPath);
if (completion.schema !== 'dclutch/product-spline-authoring-completion/v1'
    || completion.output_dir !== outputDirectory
    || completion.report !== reportPath
    || completion.report_sha256 !== digest(reportBytes)) {
  throw new Error('compiler completion does not bind the exact output and report bytes');
}

const inspection = document(inspectionPath, 'SDK inspection');
exactKeys(inspection, [
  'schema', 'report', 'key_free', 'signs', 'submits', 'input_sha256', 'registry_program',
  'product_outcome_count', 'basis_width', 'degree', 'interior_multiplicity', 'payout_scale',
  'rounding_boundary', 'semantic_basis_id', 'records', 'verified_price_gate', 'found_records',
  'partition_quality',
], 'SDK inspection');
// The partition-quality report is CHECKED, not merely tolerated. Adding a key
// to the accepted set and asserting nothing about it would turn an exact-set
// gate into a spelling test -- which is how this field broke the gate in the
// first place: the compiler grew it, and the only thing standing between the
// two was a list of names.
//
// What is bound here is the part a handoff consumer would actually rely on:
// the shares are a real distribution over the ordinary cells, the dominant
// share is the largest of them and is reported honestly, and `degenerate`
// agrees with the ceiling rather than being an independent opinion. A market
// that resolves into one cell every time is the defect this whole field
// exists to surface, so the gate refuses a report that contradicts itself
// about whether it found one.
const quality = inspection.partition_quality;
exactKeys(quality, [
  'model', 'anchor', 'volatilityBps', 'windowSlots', 'characteristicDisplacement',
  'plausibleHalfWidth', 'dominantCell', 'dominantShareBps', 'maxCellShareBps',
  'cellShareBps', 'degenerate',
], 'partition quality report');
if (!Array.isArray(quality.cellShareBps) || quality.cellShareBps.length === 0) {
  throw new Error('partition quality report states no cell shares');
}
const largestShare = Math.max(...quality.cellShareBps);
if (quality.dominantShareBps !== largestShare) {
  throw new Error('partition quality dominant share is not the largest cell share');
}
if (quality.cellShareBps[quality.dominantCell] !== largestShare) {
  throw new Error('partition quality dominant cell does not hold the dominant share');
}
if (quality.degenerate !== (largestShare >= quality.maxCellShareBps)) {
  throw new Error('partition quality degeneracy disagrees with its own ceiling');
}
if (quality.degenerate) {
  throw new Error('the handoff compiled a degenerate partition: one cell takes the market');
}

if (inspection.schema !== 'dclutch/product-spline-inspection/v1'
    || inspection.report !== reportPath
    || inspection.key_free !== true
    || inspection.signs !== false
    || inspection.submits !== false
    || inspection.input_sha256 !== digest(bytes(fixturePath))) {
  throw new Error('SDK inspection does not bind the exact key-free compiler input and report');
}
exactKeys(inspection.records, ['product', 'result_domain', 'portfolio', 'product_basis', 'price_gate'], 'inspected records');
exactKeys(inspection.found_records, ['productRecord', 'resultDomainRecord', 'portfolioRecord', 'linkedBasisRecord', 'priceGateRecord'], 'Found39 handoff');
if (Object.values(inspection.found_records).some((value) => typeof value !== 'string' || value.length === 0)) throw new Error('Found39 handoff carries an empty coordinate');

const report = Object.freeze({
  schema: 'dclutch/product-spline-handoff-smoke/v1',
  key_free: true,
  signs: false,
  submits: false,
  fixture: fixturePath,
  fixture_sha256: digest(bytes(fixturePath)),
  compiler_completion_sha256: digest(bytes(completionPath)),
  compiler_report_sha256: digest(reportBytes),
  sdk_inspection_sha256: digest(bytes(inspectionPath)),
  semantic_basis_id: inspection.semantic_basis_id,
  found_records: inspection.found_records,
});
process.stdout.write(`${JSON.stringify(report)}\n`);
