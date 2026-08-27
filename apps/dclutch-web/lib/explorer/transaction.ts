/**
 * The transaction view.
 *
 * Paste a signature; get back what the chain says ran. Three things this does
 * that a generic Solana explorer cannot:
 *
 *   - every instruction is decoded against the route the census says its magic
 *     selects, and its request body against the record spec of the same name;
 *   - the CPI frames come from `meta.innerInstructions` — the chain's account of
 *     what ran, not a client's reconstruction of what should have;
 *   - a refusal is rendered by NAME. `Custom(12294)` becomes
 *     `CoreSbfError::RentCredit` with the meaning the enum's own doc comment
 *     states, attributed to the program whose frame originated it, by the same
 *     two rules the gauntlet census credits refusals with.
 *
 * Nothing here asserts a program's identity. dClutch programs have no fixed
 * addresses — the reader selects them — so an address is labelled only when the
 * reader named it or the runtime owns it.
 */
import { VersionedTransaction } from '@solana/web3.js';

import type { SolanaRpcClient, TransactionMetaObservation } from '../rpc';
import { decodeBase58 } from './base58';
import { decodeInstructionData, programLabel, type DecodedInstruction } from './instructions';
import {
  invokedFrames,
  readReportedRefusal,
  runtimeErrorLabel,
  type InvokedFrame,
  type ReportedRefusal,
} from './refusals';

/** One account an instruction names, resolved through the expanded key list. */
export type InstructionAccount = Readonly<{
  index: number;
  address: string | null;
  label: string | null;
}>;

export type ExplorerInstruction = Readonly<{
  /** Position among the outer instructions; inner frames carry their parent's. */
  outerIndex: number;
  /** `null` for an outer instruction, the position within its CPI set otherwise. */
  innerIndex: number | null;
  stackHeight: number | null;
  programIndex: number;
  programAddress: string | null;
  programLabel: string | null;
  accounts: ReadonlyArray<InstructionAccount>;
  decoded: DecodedInstruction;
}>;

export type ExplorerTransaction = Readonly<{
  signature: string;
  slot: string;
  blockTime: string | null;
  succeeded: boolean;
  feeLamports: string;
  computeUnits: string | null;
  /** The reader's own program labels, echoed so the view can show what it used. */
  addresses: ReadonlyArray<string>;
  instructions: ReadonlyArray<ExplorerInstruction>;
  /** The refusal, named, when the transaction refused. */
  refusal: ReportedRefusal | null;
  /** A runtime refusal that carries no custom code, in the runtime's own words. */
  runtimeError: string | null;
  /** The programs the chain's logs say were invoked, in order. */
  invoked: ReadonlyArray<InvokedFrame>;
  logMessages: ReadonlyArray<string>;
  /** Why the instruction list is short or empty, when it is. */
  note: string | null;
}>;

export type ExplorerTransactionRequest = Readonly<{
  signature: string;
  programLabels?: Readonly<Record<string, string>>;
}>;

function resolveAccounts(
  indexes: ReadonlyArray<number>,
  addresses: ReadonlyArray<string>,
  labels: Readonly<Record<string, string>>,
): ReadonlyArray<InstructionAccount> {
  return Object.freeze(
    indexes.map((index) => {
      const address = addresses[index] ?? null;
      return Object.freeze({
        index,
        address,
        label: address === null ? null : programLabel(address, labels),
      });
    }),
  );
}

/**
 * The outer instructions, read from the transaction's own bytes.
 *
 * `meta` does not carry them; the serialized transaction does. When the bytes
 * fail to deserialize, this returns `null` rather than an empty list, so the
 * caller can say "could not read" instead of "there were none" — a distinction
 * the existing activity view loses.
 */
function outerInstructions(
  bytes: Uint8Array,
): ReadonlyArray<Readonly<{ programIdIndex: number; accountKeyIndexes: ReadonlyArray<number>; data: Uint8Array }>> | null {
  try {
    const decoded = VersionedTransaction.deserialize(bytes);
    return decoded.message.compiledInstructions.map((instruction) =>
      Object.freeze({
        programIdIndex: instruction.programIdIndex,
        accountKeyIndexes: Object.freeze([...instruction.accountKeyIndexes]),
        data: new Uint8Array(instruction.data),
      }),
    );
  } catch {
    return null;
  }
}

/** Build the transaction view from one finalized observation. */
export function projectTransaction(
  meta: TransactionMetaObservation,
  labels: Readonly<Record<string, string>> = {},
): ExplorerTransaction {
  const addresses = meta.accountAddresses;
  const outer = outerInstructions(meta.transactionBytes);
  const instructions: ExplorerInstruction[] = [];
  let note: string | null = null;

  if (outer === null) {
    note = 'The transaction bytes did not deserialize, so its outer instructions could not be read. The inner CPI frames below come from the chain’s own metadata and are unaffected.';
  } else if (outer.length === 0) {
    note = 'The transaction carries no instructions.';
  }

  const innerByOuter = new Map<number, typeof meta.innerInstructions>();
  for (const inner of meta.innerInstructions) {
    const held = innerByOuter.get(inner.outerIndex) ?? [];
    innerByOuter.set(inner.outerIndex, [...held, inner]);
  }

  const emitInner = (outerIndex: number) => {
    (innerByOuter.get(outerIndex) ?? []).forEach((inner, innerIndex) => {
      let data: Uint8Array;
      try {
        data = decodeBase58(inner.data);
      } catch {
        data = new Uint8Array(0);
      }
      const address = addresses[inner.programIdIndex] ?? null;
      instructions.push(
        Object.freeze({
          outerIndex,
          innerIndex,
          stackHeight: inner.stackHeight,
          programIndex: inner.programIdIndex,
          programAddress: address,
          programLabel: address === null ? null : programLabel(address, labels),
          accounts: resolveAccounts(inner.accounts, addresses, labels),
          decoded: decodeInstructionData(data),
        }),
      );
    });
  };

  if (outer === null) {
    // No outer frames to hang them from; still show every CPI the chain reports.
    for (const outerIndex of [...innerByOuter.keys()].sort((left, right) => left - right)) emitInner(outerIndex);
  } else {
    outer.forEach((instruction, outerIndex) => {
      const address = addresses[instruction.programIdIndex] ?? null;
      instructions.push(
        Object.freeze({
          outerIndex,
          innerIndex: null,
          stackHeight: 1,
          programIndex: instruction.programIdIndex,
          programAddress: address,
          programLabel: address === null ? null : programLabel(address, labels),
          accounts: resolveAccounts(instruction.accountKeyIndexes, addresses, labels),
          decoded: decodeInstructionData(instruction.data),
        }),
      );
      emitInner(outerIndex);
    });
  }

  const refusal = meta.succeeded ? null : readReportedRefusal(meta.logMessages, meta.error);
  const runtimeError = meta.succeeded || refusal !== null ? null : runtimeErrorLabel(meta.error);

  return Object.freeze({
    signature: meta.signature,
    slot: meta.slot,
    blockTime: meta.blockTime,
    succeeded: meta.succeeded,
    feeLamports: meta.feeLamports,
    computeUnits: meta.computeUnits,
    addresses,
    instructions: Object.freeze(instructions),
    refusal,
    runtimeError,
    invoked: invokedFrames(meta.logMessages),
    logMessages: meta.logMessages,
    note,
  });
}

export type ExplorerTransactionResult =
  | Readonly<{ status: 'found'; transaction: ExplorerTransaction }>
  | Readonly<{ status: 'absent'; signature: string; reason: string }>;

/** Read and project one finalized transaction. */
export async function inspectTransaction(
  client: Pick<SolanaRpcClient, 'transaction'>,
  request: ExplorerTransactionRequest,
): Promise<ExplorerTransactionResult> {
  const meta = await client.transaction(request.signature);
  if (meta === null) {
    return Object.freeze({
      status: 'absent',
      signature: request.signature,
      reason: 'The node does not serve this signature at the finalized commitment. It may be unconfirmed, dropped, or outside this node’s history.',
    });
  }
  return Object.freeze({
    status: 'found',
    transaction: projectTransaction(meta, request.programLabels ?? {}),
  });
}
