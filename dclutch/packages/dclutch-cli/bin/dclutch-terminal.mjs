#!/usr/bin/env node
// The dclutch-terminal launcher: runs the bundled CLI, or answers --help from
// the source the bundle would have been built from. The bare name `dclutch`
// belongs to the Rust reader/authoring binary in tools/dclutch-cli, which is
// the distributed artifact; this client is installed only from this checkout.
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const bundle = new URL('../dist/dclutch-terminal.mjs', import.meta.url);
if (!existsSync(fileURLToPath(bundle))) {
  // `--help` is answered by the LAUNCHER only when there is no bundle to answer
  // it -- and it answers with the page the bundle would print, not with a
  // notice standing in for one. The commands and flags have one author,
  // `src/usage.ts`, which the client renders `--help` from; that module imports
  // nothing and uses only erasable syntax, so node loads it as it stands, with
  // no bundler, no install and no second copy of the list here.
  //
  // WHY IT IS WORTH THE TROUBLE. `--help` is the question a reader asks BEFORE
  // building anything, because it is how they find out what building would give
  // them. This launcher used to answer it with three build commands and no
  // verb, so in a fresh checkout the eighteen commands the runbooks publish
  // (`markets`, `portfolio`, `intent`, `walk` and the flags beside them) were
  // each reported by `tools/gate commands` as rejected by their own program.
  // The list was there the whole time; nothing a reader could type reached it.
  //
  // Every argv carrying `--help` gets the same page, which is what the built
  // client does too: `run()` answers `values.help` before it dispatches, so
  // `markets --help` and `--help` print one page in both directions.
  if (process.argv.includes('--help') || process.argv.includes('-h')) {
    try {
      // Node reads TypeScript directly (type stripping, on by default since
      // 22.18). Where it does not, the catch below says what a reader can do
      // instead -- an unanswerable `--help` is still better than a wrong one.
      const { USAGE } = await import(new URL('../src/usage.ts', import.meta.url).href);
      process.stdout.write(`${USAGE}\n`);
      // On stderr, so the page above is exactly the client's own and a reader
      // still learns that nothing here can be RUN yet.
      process.stderr.write(
        '\nthis checkout has no built client, so the commands above cannot be run yet:\n'
        + '    npm install --prefix packages/dclutch-cli\n'
        + '    npm run build --prefix packages/dclutch-cli\n',
      );
      process.exit(0);
    } catch (error) {
      process.stdout.write(
        'usage: dclutch-terminal [global flags] <command> [args]\n'
        + '\n'
        + 'This launcher runs the bundled dclutch-terminal client, which is not built in\n'
        + `this checkout, and this node could not read the command list from source (${error instanceof Error ? error.message : String(error)}).\n`
        + 'Build it, then ask again:\n'
        + '\n'
        + '    npm install --prefix packages/dclutch-cli\n'
        + '    npm run build --prefix packages/dclutch-cli\n'
        + '    node packages/dclutch-cli/bin/dclutch-terminal.mjs --help\n',
      );
      process.exit(0);
    }
  }
  process.stderr.write('dclutch-terminal is not built yet: run `npm run build` in packages/dclutch-cli\n');
  process.exit(1);
}
await import(bundle.href);
