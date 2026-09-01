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
], 'SDK inspection');
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
