import { readFileSync } from 'node:fs';

import { loadRationalOpenWasmV1 } from './rationalOpenWasmV1';

/** Load the checked-in generated artifact without teaching production code about Node files. */
export async function loadRationalOpenWasmV1ForTest(): Promise<void> {
  const bytes = readFileSync(new URL('./generated/rationalOpenWasm/rational_open_bg.wasm', import.meta.url));
  await loadRationalOpenWasmV1(async () => new Response(bytes));
}
