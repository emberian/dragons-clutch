#!/usr/bin/env node
// The dclutch-terminal launcher: runs the bundled CLI, or says exactly how to
// build it. The bare name `dclutch` belongs to the Rust reader/authoring
// binary in tools/dclutch-cli, which is the distributed artifact; this client
// is installed only from this checkout.
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const bundle = new URL('../dist/dclutch-terminal.mjs', import.meta.url);
if (!existsSync(fileURLToPath(bundle))) {
  process.stderr.write('dclutch-terminal is not built yet: run `npm run build` in packages/dclutch-cli\n');
  process.exit(1);
}
await import(bundle.href);
