/**
 * `dclutch walk` — the funded failure walk from a terminal.
 *
 * If a market's resolution deadline has passed with no sealed observation,
 * anyone may commit the explicit failure outcome and be paid the bounty the
 * market escrowed at founding. The instruction carries only the generation
 * and terminal sequence; the frame is 22 finalized chain facts; the walker
 * signs alone and pays only the fee. Submitting early is safe: the program
 * refuses fail-closed with a code this tool names.
 *
 * The account book comes from `--book <json>` whose fields are the frame's
 * own slot names (see `@dclutch/sdk/failureWalk`); `--dry-run` prints the
 * frame and wire without submitting.
 */
import { readFileSync } from 'node:fs';

import { buildFailureWalkTransactionV1, type FailureWalkBookV1 } from '@dclutch/sdk/failureWalk';
import { COMMIT_DEADLINE_FAILURE_FRAME_V1 } from '@dclutch/sdk/generated/relayTransportV1';

import { loadKeypair, rpcClient, type CliContext } from '../context';
import { block, type Io } from '../output';
import { lamportDelta, submitAndConfirm } from '../submit';

const BOOK_FIELDS: ReadonlyArray<keyof FailureWalkBookV1> = Object.freeze([
  'resolutionProgram', 'market', 'coreProgram', 'registryActivation', 'sourceResolutionState',
  'resolutionCertificate', 'sourceMaterial', 'sourceMaterialStagingVacancy', 'windowSpec',
  'windowSpecStagingVacancy', 'productRecord', 'productRecordStagingVacancy', 'resultDomain',
  'resultDomainStagingVacancy', 'portfolioRecord', 'portfolioRecordStagingVacancy',
  'capabilityManifest', 'capabilityManifestStagingVacancy', 'failureFunding',
]);

export function decodeWalkBook(value: unknown): FailureWalkBookV1 {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) throw new Error('walk book must be one JSON object');
  const input = value as Record<string, unknown>;
  const book: Record<string, string> = {};
  for (const field of BOOK_FIELDS) {
    const entry = input[field];
    if (typeof entry !== 'string' || entry.length === 0) throw new Error(`walk book is missing ${field}`);
    book[field] = entry;
  }
  return Object.freeze(book) as FailureWalkBookV1;
}

export async function walk(context: CliContext, io: Io, env: NodeJS.ProcessEnv): Promise<number> {
  const bookPath = context.flags.book;
  if (typeof bookPath !== 'string') throw new Error('pass --book <json> with the frame addresses (fields are the frame slot names; see the client guide)');
  const book = decodeWalkBook(JSON.parse(readFileSync(bookPath, 'utf8')));
  const generation = BigInt(String(context.flags.generation ?? 1));
  const terminalSequence = BigInt(String(context.flags['terminal-sequence'] ?? 1));
  const walker = loadKeypair(context, env);
  const client = rpcClient(context);
  const blockhash = await client.latestBlockhash();
  const transaction = buildFailureWalkTransactionV1(book, walker.publicKey.toBase58(), generation, terminalSequence, blockhash.blockhash);
  if (context.flags['dry-run'] === true) {
    io.out(`walk of market ${book.market}, generation ${generation}, terminal sequence ${terminalSequence} — NOT submitted`);
    const instruction = transaction.instructions[0];
    if (instruction !== undefined) {
      for (const [index, slot] of COMMIT_DEADLINE_FAILURE_FRAME_V1.entries()) {
        const meta = instruction.keys[index];
        io.out(`  ${String(index).padStart(2)} ${slot.name.padEnd(34)} ${meta?.pubkey.toBase58() ?? '?'}${slot.writable ? '  writable' : ''}${slot.signer ? '  signer' : ''}`);
      }
    }
    const wire = transaction.serialize({ requireAllSignatures: false, verifySignatures: false });
    io.out(`  wire ${wire.length + 64} bytes signed, of 1232 — a bare legacy packet, no lookup table`);
    return 0;
  }
  transaction.sign(walker);
  const wire = transaction.serialize();
  io.out(`walking market ${book.market} to its explicit failure outcome (${wire.length} byte legacy packet)`);
  const outcome = await submitAndConfirm(client, Uint8Array.from(wire), io);
  if (!outcome.succeeded) return 1;
  if (outcome.meta !== null) {
    const delta = lamportDelta(outcome.meta, walker.publicKey.toBase58());
    block(io, [
      ['walker', walker.publicKey.toBase58()],
      ['paid', delta === null ? 'unknown' : `${delta} lamports net of the fee (fee ${outcome.meta.feeLamports})`],
      ['certificate', book.resolutionCertificate],
    ]);
  } else {
    io.out(`bounty paid to ${walker.publicKey.toBase58()}; certificate at ${book.resolutionCertificate}`);
  }
  return 0;
}
