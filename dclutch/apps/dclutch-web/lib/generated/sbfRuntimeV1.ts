// @generated from the pinned solana-sbpf, solana-syscalls and solana-program-entrypoint
// crates plus crates/dclutch-sbf-runtime/src/lib.rs; do not edit.
// Regenerate with: npm run abi:sbf-runtime

/** The runtime crate versions this vocabulary was read from. */
export const SBF_RUNTIME_VERSIONS_V1 = Object.freeze({
  sbpf: "0.23.0",
  syscalls: "4.3.0-beta.2",
  entrypoint: "3.1.1",
} as const);

export interface SbfMemoryRegionV1 {
  /** The VM's own name for the region, from its `MM_<NAME>_START` constant. */
  readonly name: string;
  readonly start: number;
}

/** Size and alignment of every region: `MM_REGION_SIZE`. */
export const SBF_MEMORY_REGION_SIZE_V1 = 4294967296 as const;

/** The SBF address space, in address order. */
export const SBF_MEMORY_REGIONS_V1: ReadonlyArray<SbfMemoryRegionV1> = Object.freeze([
  Object.freeze({ name: "rodata", start: 0 }),
  Object.freeze({ name: "bytecode", start: 4294967296 }),
  Object.freeze({ name: "stack", start: 8589934592 }),
  Object.freeze({ name: "heap", start: 12884901888 }),
  Object.freeze({ name: "input", start: 17179869184 }),
]);

/** Heap a transaction gets without asking: `HEAP_LENGTH`. */
export const SBF_DEFAULT_HEAP_BYTES_V1 = 32768 as const;
/** Largest heap frame the runtime will grant: `ADAPTER_MAX_HEAP_BYTES`. */
export const SBF_MAX_HEAP_BYTES_V1 = 262144 as const;
/** Multiple a `RequestHeapFrame` must be: `HEAP_FRAME_GRANULARITY_BYTES`. */
export const SBF_HEAP_FRAME_GRANULARITY_BYTES_V1 = 1024 as const;

export interface SbfDisplayFormatV1 {
  readonly variant: string;
  /** The variant's `thiserror` format string, with its `{…}` holes intact. */
  readonly format: string;
}

/** Every `EbpfError` the virtual machine can print. */
export const SBF_VM_ERROR_FORMATS_V1: ReadonlyArray<SbfDisplayFormatV1> = Object.freeze([
  Object.freeze({ variant: "ElfError", format: "ELF error: {0}" }),
  Object.freeze({ variant: "FunctionAlreadyRegistered", format: "function #{0} was already registered" }),
  Object.freeze({ variant: "CallDepthExceeded", format: "exceeded max BPF to BPF call depth" }),
  Object.freeze({ variant: "ExitRootCallFrame", format: "attempted to exit root call frame" }),
  Object.freeze({ variant: "DivideByZero", format: "divide by zero at BPF instruction" }),
  Object.freeze({ variant: "DivideOverflow", format: "division overflow at BPF instruction" }),
  Object.freeze({ variant: "ExecutionOverrun", format: "attempted to execute past the end of the text segment at BPF instruction" }),
  Object.freeze({ variant: "CallOutsideTextSegment", format: "callx attempted to call outside of the text segment" }),
  Object.freeze({ variant: "ExceededMaxInstructions", format: "exceeded CUs meter at BPF instruction" }),
  Object.freeze({ variant: "JitNotCompiled", format: "program has not been JIT-compiled" }),
  Object.freeze({ variant: "InvalidMemoryRegion", format: "Invalid memory region at index {0}" }),
  Object.freeze({ variant: "AccessViolation", format: "Access violation {0} {2} bytes at address {1:#x} (in {3} region)" }),
  Object.freeze({ variant: "StackAccessViolation", format: "Access violation in stack frame {3} at address {1:#x} of size {2:?}" }),
  Object.freeze({ variant: "InvalidInstruction", format: "invalid BPF instruction" }),
  Object.freeze({ variant: "UnsupportedInstruction", format: "unsupported BPF instruction" }),
  Object.freeze({ variant: "ExhaustedTextSegment", format: "Compilation exhausted text segment at BPF instruction {0}" }),
  Object.freeze({ variant: "LibcInvocationFailed", format: "Libc calling {0} {1:?} returned error code {2}" }),
  Object.freeze({ variant: "VerifierError", format: "Verifier error: {0}" }),
  Object.freeze({ variant: "SyscallError", format: "Syscall error: {0}" }),
]);

/** Every `SyscallError` a syscall can raise through the VM. */
export const SBF_SYSCALL_ERROR_FORMATS_V1: ReadonlyArray<SbfDisplayFormatV1> = Object.freeze([
  Object.freeze({ variant: "InvalidString", format: "{0}: {1:?}" }),
  Object.freeze({ variant: "Abort", format: "SBF program panicked" }),
  Object.freeze({ variant: "Panic", format: "SBF program Panicked in {0} at {1}:{2}" }),
  Object.freeze({ variant: "InvokeContextBorrowFailed", format: "Cannot borrow invoke context" }),
  Object.freeze({ variant: "MalformedSignerSeed", format: "Malformed signer seed: {0}: {1:?}" }),
  Object.freeze({ variant: "BadSeeds", format: "Could not create program address with signer seeds: {0}" }),
  Object.freeze({ variant: "ProgramNotSupported", format: "Program {0} not supported by inner instructions" }),
  Object.freeze({ variant: "UnalignedPointer", format: "Unaligned pointer" }),
  Object.freeze({ variant: "TooManySigners", format: "Too many signers" }),
  Object.freeze({ variant: "InstructionTooLarge", format: "Instruction passed to inner instruction is too large ({0} > {1})" }),
  Object.freeze({ variant: "TooManyAccounts", format: "Too many accounts passed to inner instruction" }),
  Object.freeze({ variant: "CopyOverlapping", format: "Overlapping copy" }),
  Object.freeze({ variant: "ReturnDataTooLarge", format: "Return data too large ({0} > {1})" }),
  Object.freeze({ variant: "TooManySlices", format: "Hashing too many sequences" }),
  Object.freeze({ variant: "InvalidLength", format: "InvalidLength" }),
  Object.freeze({ variant: "MaxInstructionDataLenExceeded", format: "Invoked an instruction with data that is too large ({data_len} > {max_data_len})" }),
  Object.freeze({ variant: "MaxInstructionAccountsExceeded", format: "Invoked an instruction with too many accounts ({num_accounts} > {max_accounts})" }),
  Object.freeze({ variant: "MaxInstructionAccountInfosExceeded", format: "Invoked an instruction with too many account info's ({num_account_infos} > {max_account_infos})" }),
  Object.freeze({ variant: "InvalidAttribute", format: "InvalidAttribute" }),
  Object.freeze({ variant: "InvalidPointer", format: "Invalid pointer" }),
  Object.freeze({ variant: "ArithmeticOverflow", format: "Arithmetic overflow" }),
]);
