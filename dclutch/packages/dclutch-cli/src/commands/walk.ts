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
 * own slot names (see `@dclutch/sdk/failureWalk`). This command currently
 * requires `--dry-run`: it prints the frame and wire, but does not sign or
 * submit until the walk owns a durable Submitted journal.
 */
import { readFileSync } from 'node:fs';

import { buildFailureWalkTransactionV1, type FailureWalkBookV1 } from '@dclutch/sdk/failureWalk';
import { COMMIT_DEADLINE_FAILURE_FRAME_V1 } from '@dclutch/sdk/generated/relayTransportV1';

import { loadKeypair, rpcClient, type CliContext } from '../context';
import { assertExactDevnetMutation, devnetGenesisAcknowledgment, latestExactDevnetBlockhash } from '../mutation';
import { type Io } from '../output';

export const FAILURE_WALK_MUTATION_REFUSAL_V1 =
  'failure-walk submission is not available yet. Pass --dry-run to inspect the exact frame. Submission reopens when this command persists the unsigned packet before signing, the exact signature and packet in a Submitted journal before one send, and the finalized certificate and bounty poststate afterward.';

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
  if (context.flags['dry-run'] !== true) throw new Error(FAILURE_WALK_MUTATION_REFUSAL_V1);
  const acknowledgment = devnetGenesisAcknowledgment(context);
  const client = rpcClient(context);
  await assertExactDevnetMutation(client, acknowledgment, 'failure-walk preparation');
  const bookPath = context.flags.book;
  if (typeof bookPath !== 'string') throw new Error('pass --book <json> with the frame addresses (fields are the frame slot names; see the client guide)');
  const book = decodeWalkBook(JSON.parse(readFileSync(bookPath, 'utf8')));
  const generation = BigInt(String(context.flags.generation ?? 1));
  const terminalSequence = BigInt(String(context.flags['terminal-sequence'] ?? 1));
  const walker = loadKeypair(context, env);
  const blockhash = await latestExactDevnetBlockhash(client, acknowledgment, 'failure-walk blockhash acquisition');
  const transaction = buildFailureWalkTransactionV1(book, walker.publicKey.toBase58(), generation, terminalSequence, blockhash.blockhash);
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
