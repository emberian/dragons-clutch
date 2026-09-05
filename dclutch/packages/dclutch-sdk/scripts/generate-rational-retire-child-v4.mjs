/**
 * The compact RetireReceipt child, emitted by the Rust contract that owns it.
 *
 * `lib/rationalRetireReceiptV4.ts` WRITES this record: the `DCRLHC04` family a
 * wallet signs and the `DCRRLC02` Claims child whose SHA-256 the Hot route
 * binds into the outer authority. Its only digest assertion was a regression
 * pin the encoder computed for itself, and the test said so in its own words:
 * move a row offset in the client and every assertion around it stays green
 * while the wallet signs a different child. That is not hypothetical -- the
 * vacancy group went from four accounts to five in Rust on 2026-08-29 and the
 * client stayed at four for six days, with no gate anywhere able to notice.
 *
 * So this generator does not scrape. It RUNS the contract:
 *
 *   cargo run -p dclutch-claims \
 *     --example compact_retire_child_v4
 *
 * The example builds one canonical child through the contract's own family,
 * child-header, row and request encoders and prints it as JSON.
 * `lib/rationalRetireReceiptV4.test.ts` re-encodes the same family from the
 * same named inputs, re-derives the child digest from the same rows, and
 * asserts both against these bytes -- which is the cross-boundary check the pin
 * could not be.
 *
 * `--check` re-runs the emitter and compares, so the fixture goes stale the
 * moment the Rust layout moves, exactly like every other `abi:*:verify`.
 */
import { execFileSync } from 'node:child_process';
import { readFileSync, renameSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const outputUrl = new URL('../fixtures/rational-retire-receipt-child-v4.json', import.meta.url);
const CRATE = 'dclutch-claims';
const EXAMPLE = 'compact_retire_child_v4';

const emitted = execFileSync('cargo', ['run', '--quiet', '-p', CRATE, '--example', EXAMPLE], {
  cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'],
});

/**
 * What a valid emission looks like, asked of the OUTPUT rather than assumed.
 *
 * A generator that writes whatever its producer printed is a generator that
 * publishes an empty file the day the producer changes shape. These are the
 * facts the SDK test then relies on, so they are checked before the canonical
 * file is replaced.
 */
const fixture = JSON.parse(emitted);
const HEADER_BYTES = 400;
const COORDINATE_BYTES = 272;
const FAMILY_MAGIC = Buffer.from('DCRLHC04').toString('hex');
const CHILD_MAGIC = Buffer.from('DCRRLC02').toString('hex');

function require_(condition, message) {
  if (!condition) throw new Error(`emitted child fixture ${message}`);
}

require_(fixture.headerBytes === HEADER_BYTES, `states a ${fixture.headerBytes}-byte header, not ${HEADER_BYTES}`);
require_(fixture.coordinateBytes === COORDINATE_BYTES, `states a ${fixture.coordinateBytes}-byte row, not ${COORDINATE_BYTES}`);
require_(Array.isArray(fixture.support) && fixture.support.length > 0, 'carries no support rows');
require_(fixture.family.length === HEADER_BYTES * 2, 'family is not exactly one fixed header');
require_(fixture.family.startsWith(FAMILY_MAGIC), 'family does not open with DCRLHC04');
require_(fixture.child.startsWith(CHILD_MAGIC), 'child does not open with DCRRLC02');
require_(
  fixture.child.length === (HEADER_BYTES + fixture.support.length * COORDINATE_BYTES) * 2,
  'child is not one header plus one row per support coordinate',
);
require_(/^[0-9a-f]{64}$/.test(fixture.childDigest), 'child digest is not 32 hex-encoded bytes');
for (const row of fixture.support) {
  // Five accounts, not four. The count is the fact the client got wrong.
  const accounts = [row.shardMint, row.structuredCustody, row.owner, row.position, row.admission];
  require_(accounts.every((value) => /^[0-9a-f]{64}$/.test(value)), 'has a vacancy account that is not one identity');
  require_(new Set(accounts).size === 5, 'has a vacancy row that aliases two of its five accounts');
}

if (process.argv.includes('--check')) {
  if (readFileSync(outputUrl, 'utf8') !== emitted) {
    process.stderr.write('fixtures/rational-retire-receipt-child-v4.json is stale; run npm run abi:rational-retire-child\n');
    process.exit(1);
  }
  process.stdout.write('fixtures/rational-retire-receipt-child-v4.json matches its Rust emitter\n');
} else {
  // Never redirect a generator into the canonical output: a failed run must
  // leave the last accepted fixture byte-for-byte intact.
  const staging = new URL('../fixtures/.rational-retire-receipt-child-v4.json.staging', import.meta.url);
  writeFileSync(staging, emitted);
  renameSync(staging, outputUrl);
  process.stdout.write('wrote fixtures/rational-retire-receipt-child-v4.json\n');
}
