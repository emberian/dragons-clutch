/**
 * Terminal rendering, including the one thing a chain client owes its user:
 * refusals by NAME. A bare `custom program error: 0x5042` is the protocol
 * working and the user left alone with a number; every error path here runs
 * the text through the band registry before it reaches the terminal.
 */
import { customCodeFromTransactionError, renderRefusal } from '@dclutch/sdk/refusals';

export type Io = Readonly<{
  out: (line: string) => void;
  err: (line: string) => void;
}>;

export const STDIO: Io = Object.freeze({
  out: (line: string) => process.stdout.write(`${line}\n`),
  err: (line: string) => process.stderr.write(`${line}\n`),
});

/** Two-column aligned key/value block. */
export function block(io: Io, rows: ReadonlyArray<readonly [string, string]>): void {
  const width = rows.reduce((longest, [label]) => Math.max(longest, label.length), 0);
  for (const [label, value] of rows) io.out(`  ${label.padEnd(width)}  ${value}`);
}

const CUSTOM_IN_TEXT = /custom program error: (0x[0-9a-fA-F]+|\d+)/;

/**
 * Rewrite an error message so any custom program error in it is named.
 *
 * Handles both shapes a refusal reaches a client in: the parsed JSON-RPC
 * transaction error (`{"InstructionError":[i,{"Custom":n}]}`, often embedded
 * as text in an SDK message) and the log-line form
 * `custom program error: 0x5042`.
 */
export function nameRefusals(message: string): string {
  let named = message;
  const fromText = CUSTOM_IN_TEXT.exec(message);
  if (fromText?.[1] !== undefined) {
    const code = Number(fromText[1]);
    named = `${named}\n  ${renderRefusal(code).text}`;
    return named;
  }
  const embedded = /"InstructionError":\s*\[\s*\d+\s*,\s*\{\s*"Custom":\s*(\d+)\s*\}\s*\]/.exec(message);
  if (embedded?.[1] !== undefined) {
    const code = Number(embedded[1]);
    named = `${named}\n  ${renderRefusal(code).text}`;
  }
  return named;
}

/** Name a refusal from a parsed transaction error object, when there is one. */
export function nameTransactionError(error: unknown): string | null {
  const code = customCodeFromTransactionError(error);
  if (code === null) return null;
  return renderRefusal(code).text;
}

export function fail(io: Io, error: unknown): number {
  const message = error instanceof Error ? error.message : String(error);
  io.err(`refused: ${nameRefusals(message)}`);
  return 1;
}
