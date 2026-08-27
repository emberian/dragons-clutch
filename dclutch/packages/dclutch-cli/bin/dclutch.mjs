#!/usr/bin/env node
// The dclutch launcher: runs the bundled CLI, or says exactly how to build it.
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const bundle = new URL('../dist/dclutch.mjs', import.meta.url);
if (!existsSync(fileURLToPath(bundle))) {
  process.stderr.write('dclutch is not built yet: run `npm run build` in packages/dclutch-cli\n');
  process.exit(1);
}
await import(bundle.href);
