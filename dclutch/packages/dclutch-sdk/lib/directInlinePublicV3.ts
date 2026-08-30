/**
 * The public, read-first Direct V3 surface.
 *
 * The protocol-side encoder and unsigned packet compiler still exist as
 * internal conformance tools. They are deliberately absent here: exposing a
 * packet builder without the accepted durable journal, authenticated
 * `HotExecutionAckV3`, and all ten finalized writable poststates would make a
 * partial adapter look like an operating trade client.
 */
export {
  canonicalDirectInlineLookupAddressesV3,
  encodeCompactIntentSigningMessageV2,
  encodeCompactIntentV2,
  previewDirectInlineV3,
  validateRuntimeAccountProfileV2,
} from './directInlineV3';

export type {
  CheckedHotOuterEvidenceV3,
  CompactIntentV2Input,
  DirectHotAccountMetaV3,
  DirectInlineEconomicPreviewV3,
  DirectInlineHotRouteV3,
  SignedDirectIntentV3,
} from './directInlineV3';
