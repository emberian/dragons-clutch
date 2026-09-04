#!/usr/bin/env node
// The dclutch-terminal launcher: runs the bundled CLI, or says exactly how to
// build it. The bare name `dclutch` belongs to the Rust reader/authoring
// binary in tools/dclutch-cli, which is the distributed artifact; this client
// is installed only from this checkout.
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const bundle = new URL('../dist/dclutch-terminal.mjs', import.meta.url);
if (!existsSync(fileURLToPath(bundle))) {
  // `--help` is answered by the LAUNCHER only when there is no bundle to answer
  // it, and it answers with what the launcher knows: that this is a launcher,
  // and how to build the thing that owns the command list. The commands and
  // flags have one author -- `src/main.ts`, which renders them from the table
  // `parseArgs` parses with -- so a built checkout falls straight through to it
  // and this page is never what a reader with a bundle sees.
  if (process.argv.includes('--help') || process.argv.includes('-h')) {
    process.stdout.write(
      'usage: dclutch-terminal [global flags] <command> [args]\n'
      + '\n'
      + 'This launcher runs the bundled dclutch-terminal client, which is not built in\n'
      + 'this checkout, so the command and flag list -- which the bundle owns -- cannot\n'
      + 'be printed. Build it, then ask again:\n'
      + '\n'
      + '    npm install --prefix packages/dclutch-cli\n'
      + '    npm run build --prefix packages/dclutch-cli\n'
      + '    node packages/dclutch-cli/bin/dclutch-terminal.mjs --help\n',
    );
    process.exit(0);
  }
  process.stderr.write('dclutch-terminal is not built yet: run `npm run build` in packages/dclutch-cli\n');
  process.exit(1);
}
await import(bundle.href);
