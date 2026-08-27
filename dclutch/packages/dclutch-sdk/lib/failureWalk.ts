/**
 * The funded failure walk: `CommitDeadlineFailure`, built from a terminal.
 *
 * When a market's resolution deadline passes without a sealed observation,
 * ANY wallet can walk the market to its explicit failure outcome and be paid
 * the disclosed bounty for doing so. The escrow was funded at founding
 * (liveness paid, not hoped), the capability manifest quotes the bounty, and
 * the route deliberately fits a legacy packet — it must never depend on an
 * address-lookup table a silent operator never published.
 *
 * The worker is the only signer. The instruction carries nothing but the
 * market generation and the terminal sequence: no provider, no record, no
 * observation — the whole point is that this route runs when nobody is
 * answering. Everything else is refused fail-closed by the Resolution
 * program with a code `renderRefusal` can name.
 */
import { PublicKey, Transaction, TransactionInstruction } from '@solana/web3.js';

import {
  COMMIT_DEADLINE_FAILURE_ACTION,
  COMMIT_DEADLINE_FAILURE_FRAME_V1,
  COMMIT_DEADLINE_FAILURE_GENERATION_OFFSET,
  COMMIT_DEADLINE_FAILURE_INSTRUCTION_BYTES,
  COMMIT_DEADLINE_FAILURE_TERMINAL_SEQUENCE_OFFSET,
  RELAYED_SCHEMA_VERSION,
  RELAY_ACTION_OFFSET,
  RELAY_INSTRUCTION_MAGIC,
} from './generated/relayTransportV1';

/** The three fixed addresses every walk frame ends with. */
export const CLOCK_SYSVAR_ADDRESS = 'SysvarC1ock11111111111111111111111111111111';
export const RENT_SYSVAR_ADDRESS = 'SysvarRent111111111111111111111111111111111';
export const SYSTEM_PROGRAM_ADDRESS = '11111111111111111111111111111111';

/**
 * The addresses a walk needs, keyed exactly by the frame's own slot names
 * (sysvars and the System Program are supplied by this module). All of them
 * are finalized chain facts about the market being walked; none is
 * relayer-supplied — that is what makes the walk permissionless.
 */
export type FailureWalkBookV1 = Readonly<{
  resolutionProgram: string;
  market: string;
  coreProgram: string;
  registryActivation: string;
  sourceResolutionState: string;
  resolutionCertificate: string;
  sourceMaterial: string;
  sourceMaterialStagingVacancy: string;
  windowSpec: string;
  windowSpecStagingVacancy: string;
  productRecord: string;
  productRecordStagingVacancy: string;
  resultDomain: string;
  resultDomainStagingVacancy: string;
  portfolioRecord: string;
  portfolioRecordStagingVacancy: string;
  capabilityManifest: string;
  capabilityManifestStagingVacancy: string;
  failureFunding: string;
}>;

/** Encode the exact 32-byte `CommitDeadlineFailure` instruction data. */
export function encodeCommitDeadlineFailureV1(generation: bigint, terminalSequence: bigint): Uint8Array {
  if (generation < 0n || generation > 0xffff_ffff_ffff_ffffn) throw new Error('generation must be a u64');
  if (terminalSequence <= 0n || terminalSequence > 0xffff_ffff_ffff_ffffn) throw new Error('terminal sequence must be a positive u64 — zero names no certificate');
  const out = new Uint8Array(COMMIT_DEADLINE_FAILURE_INSTRUCTION_BYTES);
  out.set(new TextEncoder().encode(RELAY_INSTRUCTION_MAGIC), 0);
  new DataView(out.buffer).setUint16(8, RELAYED_SCHEMA_VERSION, true);
  out[RELAY_ACTION_OFFSET] = COMMIT_DEADLINE_FAILURE_ACTION;
  new DataView(out.buffer).setBigUint64(COMMIT_DEADLINE_FAILURE_GENERATION_OFFSET, generation, true);
  new DataView(out.buffer).setBigUint64(COMMIT_DEADLINE_FAILURE_TERMINAL_SEQUENCE_OFFSET, terminalSequence, true);
  return out;
}

/**
 * Build the walk as one bare legacy transaction.
 *
 * Legacy deliberately: the frame validator enforces exact count, exact
 * per-slot privileges and a complete no-alias rule, and the walk's own
 * witness gate asserts it fits a legacy packet. The caller signs with the
 * worker key and pays only the transaction fee; on success the escrow pays
 * the disclosed bounty to the worker.
 */
export function buildFailureWalkTransactionV1(
  book: FailureWalkBookV1,
  worker: string,
  generation: bigint,
  terminalSequence: bigint,
  recentBlockhash: string,
): Transaction {
  const addressBySlot: Readonly<Record<string, string>> = Object.freeze({
    Worker: worker,
    Market: book.market,
    CoreProgram: book.coreProgram,
    RegistryActivation: book.registryActivation,
    SourceResolutionState: book.sourceResolutionState,
    ResolutionCertificate: book.resolutionCertificate,
    SourceMaterial: book.sourceMaterial,
    SourceMaterialStagingVacancy: book.sourceMaterialStagingVacancy,
    WindowSpec: book.windowSpec,
    WindowSpecStagingVacancy: book.windowSpecStagingVacancy,
    ProductRecord: book.productRecord,
    ProductRecordStagingVacancy: book.productRecordStagingVacancy,
    ResultDomain: book.resultDomain,
    ResultDomainStagingVacancy: book.resultDomainStagingVacancy,
    PortfolioRecord: book.portfolioRecord,
    PortfolioRecordStagingVacancy: book.portfolioRecordStagingVacancy,
    CapabilityManifest: book.capabilityManifest,
    CapabilityManifestStagingVacancy: book.capabilityManifestStagingVacancy,
    FailureFunding: book.failureFunding,
    ClockSysvar: CLOCK_SYSVAR_ADDRESS,
    RentSysvar: RENT_SYSVAR_ADDRESS,
    SystemProgram: SYSTEM_PROGRAM_ADDRESS,
  });
  const keys = COMMIT_DEADLINE_FAILURE_FRAME_V1.map((slot) => {
    const address = addressBySlot[slot.name];
    if (address === undefined) throw new Error(`walk book is missing an address for frame slot ${slot.name}`);
    return { pubkey: new PublicKey(address), isSigner: slot.signer, isWritable: slot.writable };
  });
  const instruction = new TransactionInstruction({
    programId: new PublicKey(book.resolutionProgram),
    keys,
    data: encodeCommitDeadlineFailureV1(generation, terminalSequence) as Buffer,
  });
  const transaction = new Transaction({ feePayer: new PublicKey(worker), recentBlockhash, signatures: [] });
  transaction.add(instruction);
  return transaction;
}
