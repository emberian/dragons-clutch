import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import initWasm, {
  derive_source_provider_submit_fresh_v1,
  plan_source_provider_reclaim_v1,
  verify_source_provider_submit_poststate_v1,
} from './generated/sourceProviderWasm/source_provider.js';
import {
  loadSourceProviderWasmV1,
  parseSourceProviderReclaimPlanV1,
} from './sourceProviderV1';

const root = fileURLToPath(new URL('../../..', import.meta.url));
const wasmPath = fileURLToPath(new URL('./generated/sourceProviderWasm/source_provider_bg.wasm', import.meta.url));

describe('Source provider native/WASM parity', () => {
  it('compiles the reclaim message and signer boundary byte-for-byte identically', async () => {
    const fixture = JSON.parse(execFileSync('cargo', [
      'run', '--quiet', '-p', 'dclutch-source-provider-wasm',
      '--bin', 'source-provider-parity', '--', '--fixture',
    ], { cwd: root, encoding: 'utf8' })) as { input: string; output: string };
    await initWasm({ module_or_path: readFileSync(wasmPath) });
    const wasm = plan_source_provider_reclaim_v1(fixture.input);
    expect(wasm).toBe(fixture.output);
    const plan = parseSourceProviderReclaimPlanV1(wasm);
    expect(plan.route).toBe('reclaim');
    expect(plan.requiredSigners).toHaveLength(2);
    expect(plan.instruction.accounts).toHaveLength(18);
    expect(plan.lookupTables).toEqual([]);
    expect(plan.wireBytes).toBeLessThanOrEqual(1_232);
  }, 180_000);

  it('refuses plan extensions and executes only the generated artifact identity', async () => {
    const fixture = JSON.parse(execFileSync('cargo', [
      'run', '--quiet', '-p', 'dclutch-source-provider-wasm',
      '--bin', 'source-provider-parity', '--', '--fixture',
    ], { cwd: root, encoding: 'utf8' })) as { output: string };
    expect(() => parseSourceProviderReclaimPlanV1(JSON.stringify({
      ...(JSON.parse(fixture.output) as Record<string, unknown>), extra: true,
    }))).toThrow(/unknown fields/);
    const exact = new Uint8Array(readFileSync(wasmPath));
    const loaded = await loadSourceProviderWasmV1((async () => new Response(exact)) as typeof fetch);
    expect(typeof loaded.plan_source_provider_reclaim_v1).toBe('function');
    const changed = new Uint8Array(exact);
    changed[changed.length - 1]! ^= 1;
    await expect(loadSourceProviderWasmV1((async () => new Response(changed)) as typeof fetch))
      .rejects.toThrow(/do not match the generated Rust artifact identity/);
  }, 180_000);

  it('keeps submit discovery and poststate verification native/WASM identical', async () => {
    const fresh = JSON.parse(execFileSync('cargo', [
      'run', '--quiet', '-p', 'dclutch-source-provider-wasm',
      '--bin', 'source-provider-parity', '--', '--submit-fresh-fixture',
    ], { cwd: root, encoding: 'utf8' })) as { input: string; output: string };
    const poststate = JSON.parse(execFileSync('cargo', [
      'run', '--quiet', '-p', 'dclutch-source-provider-wasm',
      '--bin', 'source-provider-parity', '--', '--submit-poststate-fixture',
    ], { cwd: root, encoding: 'utf8' })) as { input: string; output: string };
    await initWasm({ module_or_path: readFileSync(wasmPath) });
    expect(derive_source_provider_submit_fresh_v1(fresh.input)).toBe(fresh.output);
    expect(verify_source_provider_submit_poststate_v1(poststate.input)).toBe(poststate.output);
  }, 180_000);
});
