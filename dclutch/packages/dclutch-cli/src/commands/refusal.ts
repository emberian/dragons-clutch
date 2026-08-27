/**
 * `dclutch refusal <code...>` — the band registry from a terminal. Accepts
 * decimal or 0x hex, and full log lines (`custom program error: 0x5042`)
 * pasted verbatim.
 */
import { renderRefusal } from '@dclutch/sdk/refusals';

import type { Io } from '../output';

export function refusal(io: Io, args: ReadonlyArray<string>): number {
  if (args.length === 0) {
    io.err('usage: dclutch refusal <code> [<code> ...]  (decimal, 0x hex, or a pasted "custom program error" line)');
    return 1;
  }
  const text = args.join(' ');
  const pasted = /custom program error: (0x[0-9a-fA-F]+|\d+)/.exec(text);
  const codes = pasted?.[1] !== undefined ? [pasted[1]] : args;
  let failures = 0;
  for (const raw of codes) {
    const code = Number(raw);
    if (!Number.isInteger(code) || code < 0) {
      io.err(`  ${raw} is not a code`);
      failures += 1;
      continue;
    }
    io.out(`  ${renderRefusal(code).text}`);
  }
  return failures === 0 ? 0 : 1;
}
