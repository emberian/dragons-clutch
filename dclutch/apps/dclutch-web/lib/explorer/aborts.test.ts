import { describe, expect, it } from 'vitest';

import {
  SBF_DEFAULT_HEAP_BYTES_V1,
  SBF_MEMORY_REGIONS_V1,
  SBF_SYSCALL_ERROR_FORMATS_V1,
  SBF_VM_ERROR_FORMATS_V1,
} from '../generated/sbfRuntimeV1';
import {
  diagnoseAbort,
  faultingAddress,
  matchAbortSentence,
  placeAddress,
  readProgramAbort,
} from './aborts';

const TRADING = 'TradXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX';

/**
 * The two faults `entrypoint_adapter.rs:762-763` records from real runs, and
 * the offsets its own prose states. If the memory map ever moves, these stop
 * meaning what the adapter says they mean and this file goes red — which is the
 * point: the platform table is generated, and these are the tree's own
 * measurements holding it to what it was generated for.
 */
const RECORDED_HEAP_FAULTS = [
  { address: 0x30000fcf8, requested: 65_536, offset: 64_760 },
  { address: 0x30003fcf8, requested: 262_144, offset: 261_368 },
] as const;

describe('the SBF address space', () => {
  it('places both faults the trading adapter recorded in the heap, 776 bytes below the request', () => {
    for (const fault of RECORDED_HEAP_FAULTS) {
      const placed = placeAddress(fault.address);
      expect(placed.region).toBe('heap');
      expect(placed.offset).toBe(fault.offset);
      expect(fault.requested - placed.offset).toBe(776);
    }
  });

  it('declares the heap above the stack and below the input region', () => {
    const names = SBF_MEMORY_REGIONS_V1.map((region) => region.name);
    expect(names.indexOf('stack')).toBeLessThan(names.indexOf('heap'));
    expect(names.indexOf('heap')).toBeLessThan(names.indexOf('input'));
  });

  it('places an address outside every declared region as unplaced rather than guessing', () => {
    const beyond = SBF_MEMORY_REGIONS_V1[SBF_MEMORY_REGIONS_V1.length - 1].start * 4;
    expect(placeAddress(beyond).region).toBeNull();
  });
});

describe('the runtime vocabulary', () => {
  it('matches every hole-free format string to its own variant', () => {
    // A format with no holes prints exactly itself, so it is its own witness.
    // This is the check that the compiler from format string to matcher did not
    // quietly mangle punctuation — several of these carry `(`, `)` and `#`.
    for (const entry of [...SBF_VM_ERROR_FORMATS_V1, ...SBF_SYSCALL_ERROR_FORMATS_V1]) {
      if (/\{[^{}]*\}/.test(entry.format)) continue;
      expect(matchAbortSentence(entry.format)?.variant, entry.format).toBe(entry.variant);
    }
  });

  it('names the two sentences the pinned virtual machine documents', () => {
    const allocated = matchAbortSentence('Access violation writing 1 bytes at address 0x1000000000 (in allocated region)');
    expect(allocated?.variant).toBe('AccessViolation');
    expect(allocated?.captures).toEqual(['writing', '1', '0x1000000000', 'allocated']);

    const unallocated = matchAbortSentence('Access violation reading 8 bytes at address 0x1000000003 (in unallocated region)');
    expect(unallocated?.variant).toBe('AccessViolation');
  });

  it('names the syscall abort behind “SBF program panicked”', () => {
    const found = matchAbortSentence('SBF program panicked');
    expect(found?.origin).toBe('syscall');
    expect(found?.variant).toBe('Abort');
  });

  it('does not resolve a sentence the pinned vocabulary cannot print', () => {
    expect(matchAbortSentence('Access violation writing 8 bytes at address 0x30000fa58')).toBeNull();
  });

  it('reads the address out of a sentence it cannot name', () => {
    // The tolerance that matters: a validator older than the pinned crate
    // prints the same fault without the region clause, and losing the address
    // because the prose moved would lose the only actionable fact in it.
    expect(faultingAddress('Access violation writing 8 bytes at address 0x30000fa58')?.offset).toBe(64_088);
    expect(faultingAddress('Access violation writing 8 bytes at 0x30000fcf8')?.offset).toBe(64_760);
  });
});

describe('reading an abort out of the logs', () => {
  it('declines a refusal, because a program that returned a code did not fault', () => {
    expect(readProgramAbort([
      `Program ${TRADING} invoke [1]`,
      `Program ${TRADING} failed: custom program error: 0x4008`,
    ])).toBeNull();
  });

  it('returns null when nothing in the logs reports a failure', () => {
    expect(readProgramAbort([`Program ${TRADING} invoke [1]`, `Program ${TRADING} success`])).toBeNull();
  });

  it('reads the frame, the meter and the allocator line together', () => {
    const abort = readProgramAbort([
      `Program ${TRADING} invoke [1]`,
      'Program log: Error: memory allocation failed, out of memory',
      `Program ${TRADING} consumed 527665 of 1399850 compute units`,
      `Program ${TRADING} failed: SBF program panicked`,
    ]);
    expect(abort?.program).toBe(TRADING);
    expect(abort?.named?.variant).toBe('Abort');
    expect(abort?.allocatorExhausted).toBe(true);
    expect(abort?.meter).toEqual({ program: TRADING, consumed: 527_665, limit: 1_399_850 });
  });
});

describe('what a person can do about it', () => {
  const heapLogs = (address: number) => [
    `Program ${TRADING} invoke [1]`,
    `Program ${TRADING} consumed 203408 of 1400000 compute units`,
    `Program ${TRADING} failed: Access violation writing 8 bytes at address 0x${address.toString(16)}`,
  ];

  it('names the granted-versus-requested heap asymmetry rather than blaming the caller', () => {
    const abort = readProgramAbort(heapLogs(0x30000fa58));
    expect(abort).not.toBeNull();
    const diagnosis = diagnoseAbort(abort!, { heapFrameBytes: 65_536, computeUnitLimit: 1_400_000 });
    expect(diagnosis.title).toContain('the runtime had not mapped it');
    expect(diagnosis.finding).toContain('64,088');
    expect(diagnosis.finding).toContain('65,536');
    expect(diagnosis.remedy).toContain('raising the request moves the faulting address with it');
    expect(diagnosis.confidence).toBe('placed');
  });

  it('tells a transaction that asked for no heap that it can ask', () => {
    const abort = readProgramAbort(heapLogs(0x30000fa58));
    const diagnosis = diagnoseAbort(abort!, { heapFrameBytes: null, computeUnitLimit: null });
    expect(diagnosis.title).toContain('asked for no more');
    expect(diagnosis.remedy).toContain('RequestHeapFrame');
    expect(diagnosis.remedy).toContain('262,144');
  });

  it('refuses to promise more heap when the request is already at the ceiling', () => {
    const abort = readProgramAbort(heapLogs(0x300040008));
    const diagnosis = diagnoseAbort(abort!, { heapFrameBytes: 262_144, computeUnitLimit: null });
    expect(diagnosis.remedy).toContain('needs to allocate less');
  });

  it('separates a bad pointer inside the mapped heap from an exhausted one', () => {
    const abort = readProgramAbort(heapLogs(0x300000010));
    const diagnosis = diagnoseAbort(abort!, { heapFrameBytes: 65_536, computeUnitLimit: null });
    expect(diagnosis.title).toContain('faulted inside the heap it was given');
    expect(diagnosis.finding).toContain(SBF_DEFAULT_HEAP_BYTES_V1.toLocaleString('en-US'));
    expect(diagnosis.remedy).toBeNull();
  });

  it('reports the compute meter when the meter is what stopped it', () => {
    const abort = readProgramAbort([
      `Program ${TRADING} invoke [1]`,
      `Program ${TRADING} consumed 1399944 of 1400000 compute units`,
      `Program ${TRADING} failed: exceeded CUs meter at BPF instruction`,
    ]);
    const diagnosis = diagnoseAbort(abort!, { heapFrameBytes: null, computeUnitLimit: 1_400_000 });
    expect(diagnosis.title).toContain('compute units');
    expect(diagnosis.finding).toContain('1,399,944 of 1,400,000');
    expect(diagnosis.remedy).toContain('nothing left to ask for');
    expect(diagnosis.confidence).toBe('named');
  });

  it('says it cannot name a sentence the vocabulary does not carry, and shows it verbatim', () => {
    const abort = readProgramAbort([`Program ${TRADING} failed: something nobody has ever printed`]);
    const diagnosis = diagnoseAbort(abort!, { heapFrameBytes: null, computeUnitLimit: null });
    expect(diagnosis.confidence).toBe('verbatim');
    expect(diagnosis.finding).toContain('something nobody has ever printed');
    expect(diagnosis.remedy).toBeNull();
  });
});
