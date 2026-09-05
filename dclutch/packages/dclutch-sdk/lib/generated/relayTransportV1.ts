// @generated from crates/dclutch-source/src/relay/{instruction,frame}.rs and generated_relayed_abi.rs; do not edit.
// Regenerate with: npm run abi:relay-transport

export const RELAY_INSTRUCTION_MAGIC = "DCLTRIX1" as const;
export const RELAYED_SCHEMA_VERSION = 1 as const;
export const RELAY_ACTION_OFFSET = 10 as const;
export const COMMIT_DEADLINE_FAILURE_ACTION = 6 as const;
export const COMMIT_DEADLINE_FAILURE_INSTRUCTION_BYTES = 32 as const;
export const COMMIT_DEADLINE_FAILURE_GENERATION_OFFSET = 16 as const;
export const COMMIT_DEADLINE_FAILURE_TERMINAL_SEQUENCE_OFFSET = 24 as const;

export interface RelayFrameSlotV1 {
  readonly name: string;
  readonly signer: boolean;
  readonly writable: boolean;
}

export const COMMIT_DEADLINE_FAILURE_FRAME_V1: ReadonlyArray<RelayFrameSlotV1> = [
  { name: "Worker", signer: true, writable: true },
  { name: "Market", signer: false, writable: false },
  { name: "CoreProgram", signer: false, writable: false },
  { name: "RegistryActivation", signer: false, writable: false },
  { name: "SourceResolutionState", signer: false, writable: true },
  { name: "ResolutionCertificate", signer: false, writable: true },
  { name: "SourceMaterial", signer: false, writable: false },
  { name: "SourceMaterialStagingVacancy", signer: false, writable: false },
  { name: "WindowSpec", signer: false, writable: false },
  { name: "WindowSpecStagingVacancy", signer: false, writable: false },
  { name: "ProductRecord", signer: false, writable: false },
  { name: "ProductRecordStagingVacancy", signer: false, writable: false },
  { name: "ResultDomain", signer: false, writable: false },
  { name: "ResultDomainStagingVacancy", signer: false, writable: false },
  { name: "PortfolioRecord", signer: false, writable: false },
  { name: "PortfolioRecordStagingVacancy", signer: false, writable: false },
  { name: "CapabilityManifest", signer: false, writable: false },
  { name: "CapabilityManifestStagingVacancy", signer: false, writable: false },
  { name: "ResolutionFunding", signer: false, writable: true },
  { name: "ClockSysvar", signer: false, writable: false },
  { name: "RentSysvar", signer: false, writable: false },
  { name: "SystemProgram", signer: false, writable: false },
];
