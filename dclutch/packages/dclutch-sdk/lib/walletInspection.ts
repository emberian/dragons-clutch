/**
 * Public transaction inspection without signing or submission authority.
 *
 * Generic sign-and-send helpers cannot enforce a workflow's semantic owner,
 * durable phase transition, exact acknowledgement, or finalized poststates.
 * Those capabilities stay behind caller-specific journals instead of being a
 * universal escape hatch.
 */
export {
  SOLANA_PACKET_BYTES,
  acquireUnsignedTransactionDependenciesV1,
  inspectUnsignedTransactionV1,
} from './walletHandoff';

export type {
  UnsignedDependencyV1,
  UnsignedTransactionChainReportV1,
  UnsignedTransactionInspectionV1,
} from './walletHandoff';
