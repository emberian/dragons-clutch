import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync, renameSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const output = fileURLToPath(new URL('../fixtures/codeCommitmentV2.native.json', import.meta.url));
const check = process.argv.slice(2);
if (check.some((arg) => arg !== '--check') || check.length > 1) throw new Error('usage: generate-code-commitment-vectors.mjs [--check]');
const result = spawnSync('cargo', [
  'run', '--locked', '--quiet', '-p', 'dclutch-release-tool', '--example', 'code_commitment_vectors',
], { cwd: root, encoding: 'utf8', maxBuffer: 1_000_000 });
if (result.error || result.status !== 0) throw new Error(`native code commitment emitter failed: ${result.error?.message ?? result.stderr}`);
const document = JSON.parse(result.stdout);
if (document.schema !== 'dclutch-code-commitment-native-vectors-v2'
  || document.generator !== 'crates/dclutch-release-tool/examples/code_commitment_vectors.rs'
  || !Array.isArray(document.vectors) || document.vectors.length === 0
  || document.vectors.some((row) => !Number.isSafeInteger(row.length) || row.length <= 0 || !/^[0-9a-f]{64}$/.test(row.code_commitment))) {
  throw new Error('native code commitment emitter returned an invalid corpus');
}
if (check.length) {
  if (readFileSync(output, 'utf8') !== result.stdout) throw new Error('code commitment corpus differs from its native owner; regenerate it');
  process.stdout.write('code commitment corpus matches its native owner\n');
} else {
  const temporary = `${output}.tmp-${process.pid}`;
  writeFileSync(temporary, result.stdout);
  renameSync(temporary, output);
  process.stdout.write('wrote native code commitment corpus\n');
}
