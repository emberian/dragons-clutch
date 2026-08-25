import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const app = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repository = resolve(app, '../..');
const manifest = JSON.parse(readFileSync(resolve(app, 'fixtures/provenance.json'), 'utf8'));

for (const source of manifest.sources) {
  const body = readFileSync(resolve(repository, source.path));
  const observed = createHash('sha256').update(body).digest('hex');
  if (observed !== source.sha256) throw new Error(`${source.path} changed: regenerate and review the frontend fixtures`);
}

const generated = execFileSync('cargo', [
  'run', '--quiet', '--locked', '--manifest-path', resolve(app, 'fixtures/rust/Cargo.toml'),
], { cwd: repository, encoding: 'utf8' });
const committed = readFileSync(resolve(app, 'fixtures/canonical-accounts.json'), 'utf8');
if (`${generated.trim()}\n` !== committed) throw new Error('canonical-accounts.json differs from the canonical Rust encoder output');
console.log('Canonical frontend fixtures match their pinned Rust source provenance.');
