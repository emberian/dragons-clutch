import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import initWasm, {
  derive_source_readiness_base_v1,
  derive_source_close_detail_v1,
  derive_source_terminal_base_v1,
} from './generated/sourceReadinessWasm/source_readiness.js';
import { loadSourceReadinessWasmV1 } from './sourceReadinessV1';

const root = fileURLToPath(new URL('../../..', import.meta.url));
const wasmPath = fileURLToPath(new URL('./generated/sourceReadinessWasm/source_readiness_bg.wasm', import.meta.url));

describe('Source readiness native/WASM parity', () => {
  it('returns byte-for-byte identical Rust-owned coordinates for one exact Market', async () => {
    const fixture = execFileSync('cargo', [
      'run', '--quiet', '-p', 'dclutch-source-readiness-operator',
      '--bin', 'source-readiness-parity', '--', 'fixture-base',
    ], { cwd: root, encoding: 'utf8' });
    const native = execFileSync('cargo', [
      'run', '--quiet', '-p', 'dclutch-source-readiness-operator',
      '--bin', 'source-readiness-parity', '--', 'base',
    ], { cwd: root, input: fixture, encoding: 'utf8' });
    await initWasm({ module_or_path: readFileSync(wasmPath) });
    expect(derive_source_readiness_base_v1(fixture)).toBe(native);
  }, 30_000);

  it('executes only the generated blob identity and refuses one changed byte', async () => {
    const exact = new Uint8Array(readFileSync(wasmPath));
    const loaded = await loadSourceReadinessWasmV1((async () => new Response(exact)) as typeof fetch);
    expect(typeof loaded.plan_source_readiness_v1).toBe('function');
    const changed = new Uint8Array(exact);
    changed[changed.length - 1]! ^= 1;
    await expect(loadSourceReadinessWasmV1((async () => new Response(changed)) as typeof fetch))
      .rejects.toThrow(/do not match the generated Rust artifact identity/);
  });

  it('returns byte-for-byte identical terminal coordinates natively and in WASM', async () => {
    const fixture = execFileSync('cargo', [
      'run', '--quiet', '-p', 'dclutch-source-readiness-operator',
      '--bin', 'source-readiness-parity', '--', 'fixture-terminal-base',
    ], { cwd: root, encoding: 'utf8' });
    const native = execFileSync('cargo', [
      'run', '--quiet', '-p', 'dclutch-source-readiness-operator',
      '--bin', 'source-readiness-parity', '--', 'terminal-base',
    ], { cwd: root, input: fixture, encoding: 'utf8' });
    await initWasm({ module_or_path: readFileSync(wasmPath) });
    expect(derive_source_terminal_base_v1(fixture)).toBe(native);
  }, 30_000);

  it('returns byte-for-byte identical Source close coordinates natively and in WASM', async () => {
    const fixture = execFileSync('cargo', [
      'run', '--quiet', '-p', 'dclutch-source-readiness-operator',
      '--bin', 'source-readiness-parity', '--', 'fixture-close-detail',
    ], { cwd: root, encoding: 'utf8' });
    const native = execFileSync('cargo', [
      'run', '--quiet', '-p', 'dclutch-source-readiness-operator',
      '--bin', 'source-readiness-parity', '--', 'close-detail',
    ], { cwd: root, input: fixture, encoding: 'utf8' });
    await initWasm({ module_or_path: readFileSync(wasmPath) });
    expect(derive_source_close_detail_v1(fixture)).toBe(native);
  }, 30_000);
});
