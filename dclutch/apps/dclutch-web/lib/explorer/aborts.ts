/**
 * Aborts, given the treatment refusals already get.
 *
 * `refusals.ts` renders a `Custom` code by name, band and meaning. An abort has
 * none of those: the virtual machine faults, the instruction never returns a
 * code, and the RPC reports `InstructionError(n, ProgramFailedToComplete)`. All
 * three arms of `readReportedRefusal` miss — correctly, there is no custom code
 * to find — and the reader was shown the runtime's discriminant verbatim:
 *
 *     InstructionError #3: ProgramFailedToComplete
 *
 * No heap, no budget, no remedy, while
 * `Access violation writing 8 bytes at address 0x30000fcf8` sat unparsed in
 * `logMessages` two lines above it. That address is not decoration. It is a
 * coordinate in a memory map the pinned `solana-sbpf` declares, and the
 * sentence around it is that crate's own `#[error(...)]` format string. So this
 * module does for aborts what the census does for refusals: read the runtime's
 * vocabulary from `lib/generated/sbfRuntimeV1.ts` and say which word it used.
 *
 * Nothing here restates a format string, a region base, or a heap bound; every
 * one is imported. What this module adds is the second half a reader needs and
 * no authority can supply alone — the fault address compared against **this
 * transaction's own ComputeBudget declarations**, which is the difference
 * between "the program crashed" and "the program wrote inside the heap frame it
 * asked for, and the runtime mapped the default instead."
 *
 * That last case is not hypothetical. `require_extended_heap_admitted_v1`
 * (`programs/dclutch-trading-sbf/src/entrypoint_adapter.rs:740-790`) reads the
 * heap frame a transaction REQUESTED and reports it as the frame the runtime
 * GRANTED, because a program cannot observe the difference. When the two
 * disagree the route bumps past the mapped ceiling and faults, and the adapter's
 * own doc records both measured addresses. A reader who can see the request and
 * the fault together can tell that story; the program that faulted cannot.
 */
import {
  SBF_DEFAULT_HEAP_BYTES_V1,
  SBF_MAX_HEAP_BYTES_V1,
  SBF_MEMORY_REGIONS_V1,
  SBF_MEMORY_REGION_SIZE_V1,
  SBF_RUNTIME_VERSIONS_V1,
  SBF_SYSCALL_ERROR_FORMATS_V1,
  SBF_VM_ERROR_FORMATS_V1,
  type SbfDisplayFormatV1,
} from '../generated/sbfRuntimeV1';

// ------------------------------------------------------------------ matchers

/** Escape one literal run so it means itself inside a `RegExp`. */
function literal(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/**
 * A `thiserror` format string, compiled to the matcher for what it prints.
 *
 * `{0}`, `{1:#x}` and `{data_len}` are holes; everything between them is
 * printed verbatim. Splitting on the holes and escaping the rest gives a
 * pattern that moves when the runtime's wording moves, which is the whole
 * reason the format strings are imported rather than transcribed.
 */
function matcherFor(format: string): RegExp {
  const parts = format.split(/\{[^{}]*\}/g);
  return new RegExp(`^${parts.map(literal).join('(.+?)')}$`);
}

type CompiledFormat = Readonly<{ origin: 'vm' | 'syscall'; variant: string; pattern: RegExp; holes: number }>;

function compile(origin: 'vm' | 'syscall', formats: ReadonlyArray<SbfDisplayFormatV1>): ReadonlyArray<CompiledFormat> {
  return Object.freeze(formats.map((entry) => Object.freeze({
    origin,
    variant: entry.variant,
    pattern: matcherFor(entry.format),
    holes: (entry.format.match(/\{[^{}]*\}/g) ?? []).length,
  })));
}

/**
 * Every format, most-specific first.
 *
 * `EbpfError::SyscallError` prints `Syscall error: {0}` and would swallow any
 * sentence at all, so the formats are ordered by how many holes they have:
 * a match with fewer holes has more literal text agreeing, and a
 * single-hole catch-all is only reached when nothing sharper fits.
 */
const COMPILED: ReadonlyArray<CompiledFormat> = Object.freeze(
  [...compile('vm', SBF_VM_ERROR_FORMATS_V1), ...compile('syscall', SBF_SYSCALL_ERROR_FORMATS_V1)]
    .sort((left, right) => left.holes - right.holes),
);

/** One runtime sentence, resolved to the variant whose format printed it. */
export type SbfAbortMatch = Readonly<{
  /** `vm` for an `EbpfError`, `syscall` for a `SyscallError`. */
  origin: 'vm' | 'syscall';
  variant: string;
  /** What the format's holes captured, left to right. */
  captures: ReadonlyArray<string>;
}>;

/** Resolve one runtime sentence against the pinned vocabulary. */
export function matchAbortSentence(sentence: string): SbfAbortMatch | null {
  for (const format of COMPILED) {
    const found = format.pattern.exec(sentence);
    if (found === null) continue;
    return Object.freeze({
      origin: format.origin,
      variant: format.variant,
      captures: Object.freeze(found.slice(1)),
    });
  }
  return null;
}

// -------------------------------------------------------------- memory space

/** One address, placed in the SBF address space. */
export type SbfMemoryCoordinate = Readonly<{
  address: number;
  /** The VM's own name for the region, or `null` outside every declared one. */
  region: string | null;
  /** Distance from the region's base, or from the address itself when unplaced. */
  offset: number;
}>;

/** Place one virtual address in the region map the VM declares. */
export function placeAddress(address: number): SbfMemoryCoordinate {
  for (const region of SBF_MEMORY_REGIONS_V1) {
    if (address >= region.start && address < region.start + SBF_MEMORY_REGION_SIZE_V1) {
      return Object.freeze({ address, region: region.name, offset: address - region.start });
    }
  }
  return Object.freeze({ address, region: null, offset: address });
}

/**
 * The faulting address in one runtime sentence.
 *
 * Read separately from the format match on purpose. A validator older than the
 * pinned crate prints the same fact in a slightly different sentence — the tree
 * has `at 0x30000fcf8` and `at address 0x30000fa58` recorded from two different
 * runs — and an address a reader can act on should not be lost because the
 * prose around it moved. When the sentence carries several, the first is the
 * faulting one; the rest are sizes and indices.
 */
const ADDRESS = /\bat (?:address )?(0x[0-9a-fA-F]+)/;

export function faultingAddress(sentence: string): SbfMemoryCoordinate | null {
  const found = ADDRESS.exec(sentence);
  if (found === null) return null;
  const address = Number.parseInt(found[1], 16);
  if (!Number.isSafeInteger(address)) return null;
  return placeAddress(address);
}

// --------------------------------------------------------------- log reading

/** `Program <address> failed: <sentence>`, where the sentence is not a code. */
const FAILED = /^Program (\S+) failed: (.+)$/;
/** The bare form, which the outermost frame uses when it names no program. */
const BARE_FAILED = /^Program failed to complete: (.+)$/;
/** `Program <address> consumed <used> of <limit> compute units`. */
const CONSUMED = /^Program (\S+) consumed (\d+) of (\d+) compute units$/;
/**
 * The heap allocator's own last words.
 *
 * Not a `#[error]` string anywhere in the pinned crates: the SBF bump allocator
 * logs it directly before returning null, and the caller then aborts. Recorded
 * against real runs at `programs/dclutch-trading-sbf/src/hot_v3.rs:5955` and
 * `program-test/tests/direct_hot_top_level.rs:128`, which is the provenance
 * this line has. It is matched as a substring because the log prefix
 * (`Program log: `) belongs to the logger rather than to the message.
 */
const ALLOCATOR_EXHAUSTED = 'Error: memory allocation failed, out of memory';

/** What a transaction declared to the ComputeBudget program, if anything. */
export type TransactionBudget = Readonly<{
  /** Bytes of heap requested, or `null` when the transaction requested none. */
  heapFrameBytes: number | null;
  /** Compute units requested, or `null` when the transaction requested none. */
  computeUnitLimit: number | null;
}>;

/** The compute meter one frame reported, as the log states it. */
export type ComputeMeter = Readonly<{ program: string; consumed: number; limit: number }>;

/** A runtime abort, read out of one transaction's logs. */
export type ProgramAbort = Readonly<{
  /** The runtime's own sentence, with any `Program … failed: ` prefix removed. */
  sentence: string;
  /** The program whose frame reported it, when the log names one. */
  program: string | null;
  /** The variant the pinned vocabulary identifies, or `null` when none fits. */
  named: SbfAbortMatch | null;
  /** The faulting address, placed, when the sentence carries one. */
  fault: SbfMemoryCoordinate | null;
  /** Whether the heap allocator announced exhaustion before the abort. */
  allocatorExhausted: boolean;
  /** The last compute meter the logs reported, whichever frame reported it. */
  meter: ComputeMeter | null;
}>;

/**
 * Read the abort out of a transaction's logs.
 *
 * Returns `null` when the failure carries a custom program error, because that
 * is a refusal and `readReportedRefusal` owns it. The two are exclusive by
 * construction: a program that returns a code did not fault.
 */
export function readProgramAbort(logs: ReadonlyArray<string>): ProgramAbort | null {
  let sentence: string | null = null;
  let program: string | null = null;
  let allocatorExhausted = false;
  let meter: ComputeMeter | null = null;

  for (const line of logs) {
    if (line.includes(ALLOCATOR_EXHAUSTED)) allocatorExhausted = true;

    const consumed = CONSUMED.exec(line);
    if (consumed !== null) {
      const used = Number.parseInt(consumed[2], 10);
      const limit = Number.parseInt(consumed[3], 10);
      if (Number.isSafeInteger(used) && Number.isSafeInteger(limit)) {
        meter = Object.freeze({ program: consumed[1], consumed: used, limit });
      }
      continue;
    }

    const bare = BARE_FAILED.exec(line);
    if (bare !== null) {
      sentence = bare[1];
      continue;
    }

    const failed = FAILED.exec(line);
    if (failed === null) continue;
    if (failed[2].startsWith('custom program error')) return null;
    sentence = failed[2];
    program = failed[1];
  }

  if (sentence === null) return null;
  return Object.freeze({
    sentence,
    program,
    named: matchAbortSentence(sentence),
    fault: faultingAddress(sentence),
    allocatorExhausted,
    meter,
  });
}

// ----------------------------------------------------------------- diagnosis

/** What happened, and what a person can do about it. */
export type AbortDiagnosis = Readonly<{
  /** A short sentence naming the failure, in the reader's terms. */
  title: string;
  /** The facts the reading rests on, so a reader can check it. */
  finding: string;
  /** The next act available to whoever is holding this transaction. */
  remedy: string | null;
  /** How firmly this is claimed: `named` when the pinned vocabulary matched. */
  confidence: 'named' | 'placed' | 'verbatim';
}>;

function bytes(count: number): string {
  return count.toLocaleString('en-US');
}

/**
 * Diagnose one abort against the transaction that suffered it.
 *
 * The budget is the other half. `Access violation … at 0x30000fcf8` alone says
 * a program wrote somewhere it could not; the same address next to a
 * `RequestHeapFrame(65536)` says the program wrote 776 bytes below the ceiling
 * it asked for — which is a different accusation, aimed at a different party,
 * with a different remedy.
 */
export function diagnoseAbort(abort: ProgramAbort, budget: TransactionBudget): AbortDiagnosis {
  const requested = budget.heapFrameBytes;
  const heapFault = abort.fault !== null && abort.fault.region === 'heap' ? abort.fault : null;

  if (heapFault !== null) {
    if (requested !== null && heapFault.offset >= SBF_DEFAULT_HEAP_BYTES_V1 && heapFault.offset < requested) {
      return Object.freeze({
        title: 'The program wrote inside the heap frame this transaction asked for, and the runtime had not mapped it.',
        finding: `The fault is at heap offset ${bytes(heapFault.offset)}, which is above the ${bytes(SBF_DEFAULT_HEAP_BYTES_V1)} bytes every transaction is mapped without asking and below the ${bytes(requested)} this transaction requested. A program cannot observe the difference between a heap frame the runtime accepted and one it applied, so the route bumped from a ceiling that was not there.`,
        remedy: `Not a caller error, and not fixable by asking for more: raising the request moves the faulting address with it. Either the route must fit in ${bytes(SBF_DEFAULT_HEAP_BYTES_V1)} bytes, or the transaction is being executed by a runtime that is not honouring RequestHeapFrame — check the same instruction against a validator that does.`,
        confidence: abort.named === null ? 'placed' : 'named',
      });
    }
    if (heapFault.offset >= SBF_DEFAULT_HEAP_BYTES_V1 && requested === null) {
      return Object.freeze({
        title: 'The program ran past the heap it was given, and this transaction asked for no more.',
        finding: `The fault is at heap offset ${bytes(heapFault.offset)}; without a ComputeBudget RequestHeapFrame a transaction is mapped ${bytes(SBF_DEFAULT_HEAP_BYTES_V1)} bytes.`,
        remedy: `Add a ComputeBudget RequestHeapFrame ahead of this instruction. The runtime grants up to ${bytes(SBF_MAX_HEAP_BYTES_V1)} bytes, in multiples of 1,024.`,
        confidence: abort.named === null ? 'placed' : 'named',
      });
    }
    if (requested !== null && heapFault.offset >= requested) {
      return Object.freeze({
        title: 'The program ran past the heap frame it asked for.',
        finding: `The fault is at heap offset ${bytes(heapFault.offset)}, above the ${bytes(requested)} bytes this transaction requested.`,
        remedy: requested >= SBF_MAX_HEAP_BYTES_V1
          ? `The request is already at the runtime's ceiling of ${bytes(SBF_MAX_HEAP_BYTES_V1)} bytes. This route needs to allocate less, not ask for more.`
          : `Raise the RequestHeapFrame. The runtime grants up to ${bytes(SBF_MAX_HEAP_BYTES_V1)} bytes, in multiples of 1,024.`,
        confidence: abort.named === null ? 'placed' : 'named',
      });
    }
    return Object.freeze({
      title: 'The program faulted inside the heap it was given.',
      finding: `The fault is at heap offset ${bytes(heapFault.offset)}, inside the ${bytes(SBF_DEFAULT_HEAP_BYTES_V1)} bytes mapped by default. This is a bad pointer or an unmapped sub-range, not an exhausted heap.`,
      remedy: null,
      confidence: abort.named === null ? 'placed' : 'named',
    });
  }

  if (abort.allocatorExhausted) {
    return Object.freeze({
      title: 'The heap allocator ran out and the program aborted rather than refusing.',
      finding: requested === null
        ? `The transaction declares no ComputeBudget RequestHeapFrame, so the program had the ${bytes(SBF_DEFAULT_HEAP_BYTES_V1)}-byte default.`
        : `The transaction requested ${bytes(requested)} bytes of heap.`,
      remedy: requested !== null && requested >= SBF_MAX_HEAP_BYTES_V1
        ? `The request is already at the runtime's ceiling of ${bytes(SBF_MAX_HEAP_BYTES_V1)} bytes.`
        : `Raise or add a ComputeBudget RequestHeapFrame; the runtime grants up to ${bytes(SBF_MAX_HEAP_BYTES_V1)} bytes, in multiples of 1,024.`,
      confidence: abort.named === null ? 'placed' : 'named',
    });
  }

  if (abort.named?.variant === 'ExceededMaxInstructions') {
    const meter = abort.meter;
    return Object.freeze({
      title: 'The program ran out of compute units.',
      finding: meter === null
        ? 'The logs report no compute meter for the frame that stopped.'
        : `The last frame reported ${bytes(meter.consumed)} of ${bytes(meter.limit)} units.`,
      remedy: budget.computeUnitLimit === null
        ? 'The transaction declares no ComputeBudget SetComputeUnitLimit, so each instruction was given the per-instruction default. Declare one.'
        : `The transaction requested ${bytes(budget.computeUnitLimit)} units. 1,400,000 is the runtime's maximum, so above that there is nothing left to ask for and the route itself has to cost less.`,
      confidence: 'named',
    });
  }

  if (abort.named?.variant === 'CallDepthExceeded') {
    return Object.freeze({
      title: 'The program exceeded the BPF-to-BPF call depth.',
      finding: 'A call chain inside one program ran deeper than the virtual machine allows. This is a property of the program, not of the transaction.',
      remedy: null,
      confidence: 'named',
    });
  }

  if (abort.fault !== null) {
    const region = abort.fault.region;
    return Object.freeze({
      title: region === null
        ? 'The program addressed memory outside every mapped region.'
        : `The program faulted in the ${region} region.`,
      finding: region === null
        ? `Address 0x${abort.fault.address.toString(16)} falls in no region the virtual machine declares.`
        : `The fault is at ${region} offset ${bytes(abort.fault.offset)}.`,
      remedy: null,
      confidence: abort.named === null ? 'placed' : 'named',
    });
  }

  if (abort.named !== null) {
    return Object.freeze({
      title: `The runtime aborted: ${abort.named.origin === 'vm' ? 'EbpfError' : 'SyscallError'}::${abort.named.variant}.`,
      finding: abort.sentence,
      remedy: null,
      confidence: 'named',
    });
  }

  return Object.freeze({
    title: 'The runtime aborted and this client will not guess why.',
    finding: `No format string in the pinned runtime vocabulary (solana-sbpf ${SBF_RUNTIME_VERSIONS_V1.sbpf}, solana-syscalls ${SBF_RUNTIME_VERSIONS_V1.syscalls}) prints this sentence, so it is shown in the runtime's own words: “${abort.sentence}”.`,
    remedy: null,
    confidence: 'verbatim',
  });
}
